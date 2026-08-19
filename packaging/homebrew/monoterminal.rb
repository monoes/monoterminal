class Monoterminal < Formula
  desc "Modern terminal session management daemon"
  homepage "https://github.com/monoterminal/monoterminal"
  url "https://github.com/monoterminal/monoterminal/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_ACTUAL_SHA256_HASH_AFTER_TAG_CREATION"
  license "MIT"
  head "https://github.com/monoterminal/monoterminal.git", branch: "main"

  # Dependencies
  depends_on "rust" => :build
  depends_on "protobuf"

  # Service management (Homebrew's built-in service DSL)
  service do
    run [opt_bin/"monoterminal-master", "--launchd"]
    working_dir "/Library/Application Support/MONOTERMINAL"
    environment_variables RUST_LOG: "info", TERM: "xterm-256color"
    keep_alive crashed: true, successful_exit: false
    process_type :adaptive
    error_log_path "/Library/Logs/MONOTERMINAL/stderr.log"
    log_path "/Library/Logs/MONOTERMINAL/stdout.log"
  end

  def install
    # Build release binary
    system "cargo", "build", "--release", "--workspace"

    # Install binary
    bin.install "target/release/monoterminal-master"

    # Install launchd plist (for manual service installation if needed)
    # Homebrew's service block handles automatic installation
    # This is a backup for manual setup
    (prefix/"etc/launchd").install "templates/launchd/com.monoterminal.master.plist"

    # Install documentation
    doc.install "README.md", "LICENSE"
  end

  def post_install
    # Create data directory
    system "sudo", "mkdir", "-p", "/Library/Application Support/MONOTERMINAL"
    system "sudo", "mkdir", "-p", "/Library/Logs/MONOTERMINAL"

    # Create service user (macOS convention: underscore prefix)
    unless system "dscl", ".", "-read", "/Users/_monoterminal", ">", "/dev/null", "2>&1"
      # Create group first
      system "sudo", "dscl", ".", "-create", "/Groups/_monoterminal"
      system "sudo", "dscl", ".", "-create", "/Groups/_monoterminal", "PrimaryGroupID", "299"
      system "sudo", "dscl", ".", "-create", "/Groups/_monoterminal", "RealName", "MONOTERMINAL Service Group"

      # Create user
      system "sudo", "dscl", ".", "-create", "/Users/_monoterminal"
      system "sudo", "dscl", ".", "-create", "/Users/_monoterminal", "UserShell", "/usr/bin/false"
      system "sudo", "dscl", ".", "-create", "/Users/_monoterminal", "RealName", "MONOTERMINAL Service User"
      system "sudo", "dscl", ".", "-create", "/Users/_monoterminal", "UniqueID", "299"
      system "sudo", "dscl", ".", "-create", "/Users/_monoterminal", "PrimaryGroupID", "299"
      system "sudo", "dscl", ".", "-create", "/Users/_monoterminal", "NFSHomeDirectory", "/var/empty"
    end

    # Set ownership
    system "sudo", "chown", "-R", "_monoterminal:_monoterminal", "/Library/Application Support/MONOTERMINAL"
    system "sudo", "chown", "-R", "_monoterminal:_monoterminal", "/Library/Logs/MONOTERMINAL"
    system "sudo", "chmod", "750", "/Library/Application Support/MONOTERMINAL"
    system "sudo", "chmod", "750", "/Library/Logs/MONOTERMINAL"
  end

  def caveats
    <<~EOS
      MONOTERMINAL has been installed successfully.

      To start the service:
        brew services start monoterminal

      Or to run manually:
        monoterminal-master --launchd

      Configuration: /Library/Application Support/MONOTERMINAL/config.toml
      Logs: /Library/Logs/MONOTERMINAL/

      The service runs as user '_monoterminal' for security isolation.

      NOTE: Service user creation requires sudo privileges during installation.
      You may be prompted for your password.
    EOS
  end

  test do
    # Basic smoke test: verify binary exists and responds to --version
    assert_match "monoterminal", shell_output("#{bin}/monoterminal-master --version")
  end
end
