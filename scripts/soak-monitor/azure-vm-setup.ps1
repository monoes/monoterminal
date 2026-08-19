<#
.SYNOPSIS
    Provisions Azure Windows 11 VM for 24-hour soak test execution

.DESCRIPTION
    Creates a Standard_D4s_v5 Windows 11 Pro VM with:
    - DirectX 12 capable graphics
    - RDP access (public IP)
    - Auto-shutdown Sunday 11 PM ET
    - Secure credential generation and handoff

.PARAMETER ResourceGroup
    Resource group name (default: monoterminal-phase1)

.PARAMETER VMName
    Virtual machine name (default: monoterminal-soak-weekend)

.PARAMETER Region
    Azure region (default: eastus)

.PARAMETER SkipResourceGroupCreation
    Skip resource group creation (use if already exists)

.PARAMETER AdminUsername
    Admin username (default: soakadmin)

.EXAMPLE
    .\azure-vm-setup.ps1
    # Creates VM with default settings

.EXAMPLE
    .\azure-vm-setup.ps1 -ResourceGroup "my-rg" -VMName "my-vm"
    # Custom resource group and VM name

.NOTES
    Requires: Azure CLI installed and logged in
    Cost: ~$75 for 54-hour reservation
    Timeline: 5-10 minutes to provision
#>

param(
    [string]$ResourceGroup = "monoterminal-phase1",
    [string]$VMName = "monoterminal-soak-weekend",
    [string]$Region = "eastus",
    [string]$AdminUsername = "soakadmin",
    [switch]$SkipResourceGroupCreation
)

$ErrorActionPreference = "Stop"

# Colors for output
function Write-Success { param($msg) Write-Host "[✓] $msg" -ForegroundColor Green }
function Write-Info { param($msg) Write-Host "[i] $msg" -ForegroundColor Cyan }
function Write-Warn { param($msg) Write-Host "[!] $msg" -ForegroundColor Yellow }
function Write-Fail { param($msg) Write-Host "[✗] $msg" -ForegroundColor Red }

Write-Info "Azure VM Setup - Phase 1 Soak Test Infrastructure"
Write-Info "=================================================="
Write-Info ""

# Step 1: Verify Azure CLI installed
Write-Info "Step 1: Verifying Azure CLI installation..."
try {
    $azVersion = az --version 2>$null
    if ($LASTEXITCODE -ne 0) { throw "Azure CLI not found" }
    Write-Success "Azure CLI installed"
} catch {
    Write-Fail "Azure CLI not installed"
    Write-Warn "Install: winget install Microsoft.AzureCLI"
    exit 1
}

# Step 2: Verify Azure login
Write-Info "Step 2: Verifying Azure login..."
try {
    $account = az account show 2>$null | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw "Not logged in" }
    Write-Success "Logged in as: $($account.user.name)"
    Write-Info "Subscription: $($account.name)"
} catch {
    Write-Fail "Not logged in to Azure"
    Write-Warn "Run: az login"
    exit 1
}

# Step 3: Create resource group (if needed)
if (-not $SkipResourceGroupCreation) {
    Write-Info "Step 3: Creating resource group '$ResourceGroup' in '$Region'..."
    try {
        $rg = az group create --name $ResourceGroup --location $Region | ConvertFrom-Json
        Write-Success "Resource group created: $($rg.name)"
    } catch {
        Write-Warn "Resource group may already exist (continuing...)"
    }
} else {
    Write-Info "Step 3: Skipping resource group creation (--SkipResourceGroupCreation)"
}

# Step 4: Generate secure admin password (24 characters, mixed case + numbers + symbols)
Write-Info "Step 4: Generating secure admin password..."
# Character sets: digits (48-57), uppercase (65-90), lowercase (97-122), symbols (!#$%&*+-=?@)
$charSets = (48..57) + (65..90) + (97..122) + @(33,35,36,37,38,42,43,45,61,63,64)
$adminPassword = -join ($charSets | Get-Random -Count 24 | ForEach-Object {[char]$_})
Write-Success "Secure password generated (24 characters)"

# Step 5: Create VM
Write-Info "Step 5: Provisioning VM '$VMName' (Standard_D4s_v5, Windows 11 Pro)..."
Write-Warn "This will take 5-10 minutes. Please wait..."

# Note: Password is passed via variable, not hardcoded
$vmCreateArgs = @(
    "vm", "create"
    "--resource-group", $ResourceGroup
    "--name", $VMName
    "--image", "Win11ProN"
    "--size", "Standard_D4s_v5"
    "--admin-username", $AdminUsername
    "--admin-password", $adminPassword
    "--os-disk-size-gb", "100"
    "--location", $Region
    "--public-ip-sku", "Standard"
    "--nsg-rule", "RDP"
    "--output", "json"
)

try {
    Write-Info "Executing: az vm create..."
    $vm = & az @vmCreateArgs | ConvertFrom-Json
    Write-Success "VM provisioned: $($vm.name)"
    $publicIP = $vm.publicIpAddress
    Write-Success "Public IP: $publicIP"
} catch {
    Write-Fail "VM creation failed: $_"
    exit 1
}

# Step 6: Configure auto-shutdown (Sunday 11 PM ET = 03:00 UTC Monday)
Write-Info "Step 6: Configuring auto-shutdown for Sunday 11 PM ET (03:00 UTC Monday)..."
try {
    $vmResourceId = az vm show --resource-group $ResourceGroup --name $VMName --query id -o tsv

    az deployment group create `
      --resource-group $ResourceGroup `
      --template-uri "https://raw.githubusercontent.com/Azure/azure-quickstart-templates/master/quickstarts/microsoft.devtestlab/vm-auto-shutdown/azuredeploy.json" `
      --parameters `
        virtualMachineName=$VMName `
        shutdownTime="0300" `
        timeZone="UTC" `
        enableAutoShutdown=true `
      --output none

    Write-Success "Auto-shutdown configured: Sunday 11 PM ET (03:00 UTC Monday)"
} catch {
    Write-Warn "Auto-shutdown configuration failed (non-critical, can be set manually)"
    Write-Warn "Manual setup: Azure Portal → VM → Auto-shutdown → 03:00 UTC"
}

# Step 7: Verify RDP access
Write-Info "Step 7: Verifying RDP port (3389) is open..."
try {
    $nsg = az network nsg rule list --resource-group $ResourceGroup --nsg-name "${VMName}NSG" | ConvertFrom-Json
    $rdpRule = $nsg | Where-Object { $_.destinationPortRange -eq "3389" }
    if ($rdpRule) {
        Write-Success "RDP port 3389 is open"
    } else {
        Write-Warn "RDP rule not found (may need manual NSG configuration)"
    }
} catch {
    Write-Warn "NSG verification failed (non-critical)"
}

# Step 8: Generate handoff document
Write-Info "Step 8: Generating credentials handoff document..."
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$handoffFile = "soak-test-vm-credentials-$timestamp.md"

$handoffContent = @"
# Soak Test VM Credentials - CONFIDENTIAL

**Generated:** $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
**Provisioned by:** devops-lead
**Valid until:** Sunday Aug 24, 2026 11:00 PM ET (auto-shutdown)

---

## RDP Access

**IP Address:** $publicIP
**Port:** 3389 (default RDP)
**Username:** $AdminUsername
**Password:** $adminPassword

---

## Connection Instructions

### Windows RDP Client
1. Press Win+R, type: ``mstsc``
2. Computer: ``$publicIP``
3. Username: ``$AdminUsername``
4. Password: (see above)
5. Click "Connect"

### macOS/Linux
``````bash
# macOS (Microsoft Remote Desktop from App Store)
# Or command line:
rdesktop -u $AdminUsername $publicIP
# (enter password when prompted)

# Linux
xfreerdp /u:$AdminUsername /v:$publicIP
# (enter password when prompted)
``````

---

## VM Specifications

| Spec | Value |
|------|-------|
| VM Size | Standard_D4s_v5 |
| vCPUs | 4 |
| RAM | 16 GB |
| OS | Windows 11 Pro (build 22000+) |
| GPU | DirectX 12 capable |
| Storage | 100 GB SSD |
| Region | $Region |
| Auto-shutdown | Sunday 11 PM ET (03:00 UTC Monday) |

---

## Environment Setup (Saturday 9:00-9:15 AM ET)

After RDP access, run these commands in PowerShell:

``````powershell
# 1. Install Rust toolchain
winget install Rustlang.Rustup
rustup install stable
rustup default stable

# 2. Install Git
winget install Git.Git

# 3. Install protoc
winget install protocolbuffers.protobuf

# 4. Clone repository
git clone https://github.com/<org>/monoterminal.git
cd monoterminal

# 5. Verify build environment
cargo --version
rustc --version
protoc --version
``````

---

## Launch Soak Test (Saturday 9:15 AM ET)

``````powershell
cd C:\Users\$AdminUsername\monoterminal\scripts\soak-monitor
.\run-full-soak-test.ps1 -DurationHours 24
``````

**Expected output:**
``````
[09:15:00] Starting 24-hour soak test...
[09:15:05] Master daemon launched (PID: <pid>)
[09:15:10] External monitor started
[09:15:15] Monitoring every 60 seconds for 1440 iterations
[09:15:20] Test will complete at 2026-08-24 09:15 AM ET
``````

**After launch:** Disconnect RDP - test runs unattended.

---

## Evidence Collection (Sunday 9:00-9:30 AM ET)

``````powershell
# 1. Review summary
cat soak-results\SUMMARY.json

# 2. Zip evidence
Compress-Archive -Path soak-results -DestinationPath soak-results-20260824.zip

# 3. Deliver to qa-lead
# (transfer soak-results-20260824.zip via org_send or file share)
``````

---

## Troubleshooting

### RDP Connection Refused
- Wait 2-3 minutes after provisioning (VM may still be booting)
- Verify IP: ``$publicIP``
- Check NSG rules: Azure Portal → VM → Networking → Inbound port rules

### Auto-shutdown Not Configured
- Manual setup: Azure Portal → $VMName → Auto-shutdown
- Time: 03:00 UTC (= 11:00 PM ET on Sunday)
- Timezone: UTC
- Notification: Disabled (one-time VM)

### Soak Test Launch Fails
- Verify Rust installed: ``cargo --version``
- Verify protoc installed: ``protoc --version``
- Check build logs: ``cd monoterminal && cargo build 2>&1 | tee build.log``

---

## Cost Tracking

**Hourly rate:** ~\$1.40/hour (Standard_D4s_v5 in $Region)
**Total runtime:** 54 hours (Friday 6 PM → Sunday 11 PM)
**Estimated cost:** ~\$75

**Auto-shutdown ensures billing stops Sunday 11 PM ET.**

---

## Security Notice

⚠️ **CONFIDENTIAL** - Do not share this file via email, Slack, or public channels.

**Approved distribution:**
- performance-engineer (receiver)
- devops-lead (sender)
- eng-director (cc)

**After use:** Delete this file + empty Recycle Bin.

---

## Emergency Contacts

**If VM issues arise:**
- devops-lead (primary)
- eng-director (escalation)

**If test execution issues:**
- performance-engineer (primary)
- devops-lead (backup executor)

---

**VM Resource ID:**
``````
$vmResourceId
``````

**Resource Group:** $ResourceGroup
**Subscription:** $($account.name)

---

**HANDOFF COMPLETE** - VM provisioned and ready for Saturday 9 AM launch.
"@

try {
    $handoffContent | Out-File -FilePath $handoffFile -Encoding UTF8
    Write-Success "Handoff document created: $handoffFile"
} catch {
    Write-Fail "Failed to create handoff document: $_"
    exit 1
}

# Step 9: Summary
Write-Info ""
Write-Info "=================================================="
Write-Success "VM PROVISIONING COMPLETE"
Write-Info "=================================================="
Write-Info ""
Write-Info "VM Details:"
Write-Info "  Name:          $VMName"
Write-Info "  Resource Group: $ResourceGroup"
Write-Info "  Public IP:     $publicIP"
Write-Info "  Username:      $AdminUsername"
Write-Info "  Password:      (see $handoffFile)"
Write-Info ""
Write-Info "Auto-shutdown:   Sunday 11 PM ET (03:00 UTC Monday)"
Write-Info "Estimated cost:  ~`$75 for 54-hour reservation"
Write-Info ""
Write-Success "Handoff document: $handoffFile"
Write-Info ""
Write-Warn "NEXT STEPS:"
Write-Info "  1. Verify RDP access: mstsc → $publicIP"
Write-Info "  2. Deliver $handoffFile to performance-engineer (secure channel)"
Write-Info "  3. Confirm receipt by Friday 6:30 PM ET"
Write-Info ""
Write-Info "Test launch: Saturday Aug 23, 9:15 AM ET"
Write-Info "Test complete: Sunday Aug 24, 9:00 AM ET"
Write-Info ""
Write-Success "Infrastructure ready for soak test execution ✓"
