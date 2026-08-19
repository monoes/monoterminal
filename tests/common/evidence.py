"""
Evidence Collection Utilities for E2E Tests
Automates capture of screenshots, logs, WebSocket traffic, and performance metrics
"""

import json
import time
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Optional, Any
import asyncio


class EvidenceCollector:
    """
    Collects and organizes test evidence for Phase 1 gate criteria verification.
    """

    def __init__(self, evidence_dir: Path, test_name: str):
        self.evidence_dir = evidence_dir
        self.test_name = test_name
        self.test_dir = evidence_dir / test_name
        self.test_dir.mkdir(parents=True, exist_ok=True)

        self.start_time = datetime.now()
        self.metrics: List[Dict] = []
        self.websocket_log: List[Dict] = []
        self.screenshots: List[Path] = []
        self.logs: List[str] = []

    def log(self, message: str, level: str = "INFO"):
        """Log a message with timestamp."""
        timestamp = datetime.now().isoformat()
        log_entry = f"[{timestamp}] [{level}] {message}"
        self.logs.append(log_entry)
        print(log_entry)

    def record_metric(self, name: str, value: float, unit: str = ""):
        """Record a performance metric."""
        metric = {
            "timestamp": datetime.now().isoformat(),
            "name": name,
            "value": value,
            "unit": unit,
        }
        self.metrics.append(metric)

    def record_websocket_message(
        self,
        direction: str,
        message_type: str,
        payload_size: int,
        sequence_number: Optional[int] = None
    ):
        """Record WebSocket message for traffic analysis."""
        entry = {
            "timestamp": datetime.now().isoformat(),
            "direction": direction,  # "sent" or "received"
            "message_type": message_type,
            "payload_size": payload_size,
            "sequence_number": sequence_number,
        }
        self.websocket_log.append(entry)

    async def capture_screenshot(
        self,
        page,
        name: str,
        full_page: bool = False
    ) -> Path:
        """Capture screenshot and save to evidence directory."""
        screenshot_path = self.test_dir / f"{name}.png"
        await page.screenshot(path=screenshot_path, full_page=full_page)
        self.screenshots.append(screenshot_path)
        self.log(f"Screenshot captured: {screenshot_path}")
        return screenshot_path

    def save_text_file(self, name: str, content: str) -> Path:
        """Save text content to evidence file."""
        file_path = self.test_dir / f"{name}.txt"
        file_path.write_text(content, encoding="utf-8")
        self.log(f"Text file saved: {file_path}")
        return file_path

    def save_json(self, name: str, data: Any) -> Path:
        """Save JSON data to evidence file."""
        file_path = self.test_dir / f"{name}.json"
        with open(file_path, "w") as f:
            json.dump(data, f, indent=2)
        self.log(f"JSON file saved: {file_path}")
        return file_path

    def finalize(self) -> Path:
        """
        Finalize evidence collection and generate summary report.
        Returns path to the summary report.
        """
        end_time = datetime.now()
        duration = (end_time - self.start_time).total_seconds()

        # Save metrics
        if self.metrics:
            self.save_json("metrics", self.metrics)

        # Save WebSocket log
        if self.websocket_log:
            self.save_json("websocket-traffic", self.websocket_log)

        # Save test log
        if self.logs:
            self.save_text_file("test-log", "\n".join(self.logs))

        # Generate summary
        summary = {
            "test_name": self.test_name,
            "start_time": self.start_time.isoformat(),
            "end_time": end_time.isoformat(),
            "duration_seconds": duration,
            "metrics_count": len(self.metrics),
            "websocket_messages": len(self.websocket_log),
            "screenshots_count": len(self.screenshots),
            "screenshots": [str(p.relative_to(self.evidence_dir)) for p in self.screenshots],
        }

        summary_path = self.save_json("summary", summary)

        # Generate HTML report
        report_path = self.generate_html_report(summary, duration)

        self.log(f"Evidence collection finalized: {self.test_dir}")
        return report_path

    def generate_html_report(self, summary: Dict, duration: float) -> Path:
        """Generate HTML evidence report."""
        report_path = self.test_dir / "evidence-report.html"

        # Build screenshots gallery HTML
        screenshots_html = ""
        for screenshot in self.screenshots:
            rel_path = screenshot.relative_to(self.test_dir)
            screenshots_html += f"""
            <div class="screenshot">
                <h3>{rel_path.stem}</h3>
                <img src="{rel_path.name}" alt="{rel_path.stem}">
            </div>
            """

        # Build metrics table HTML
        metrics_html = ""
        if self.metrics:
            metrics_html = "<table><tr><th>Metric</th><th>Value</th><th>Unit</th></tr>"
            for metric in self.metrics[-20:]:  # Last 20 metrics
                metrics_html += f"""
                <tr>
                    <td>{metric['name']}</td>
                    <td>{metric['value']:.2f}</td>
                    <td>{metric.get('unit', '')}</td>
                </tr>
                """
            metrics_html += "</table>"

        html_content = f"""<!DOCTYPE html>
<html>
<head>
    <title>E2E Test Evidence: {self.test_name}</title>
    <style>
        body {{
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            margin: 0;
            padding: 20px;
            background: #1e1e1e;
            color: #d4d4d4;
        }}
        .header {{
            background: #2d2d30;
            padding: 20px;
            border-radius: 8px;
            margin-bottom: 20px;
        }}
        h1 {{
            color: #4ec9b0;
            margin: 0;
        }}
        h2 {{
            color: #569cd6;
            border-bottom: 2px solid #569cd6;
            padding-bottom: 10px;
        }}
        .metadata {{
            display: grid;
            grid-template-columns: 200px 1fr;
            gap: 10px;
            margin: 20px 0;
        }}
        .metadata .label {{
            font-weight: bold;
            color: #4ec9b0;
        }}
        .screenshots {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(400px, 1fr));
            gap: 20px;
            margin: 20px 0;
        }}
        .screenshot {{
            background: #2d2d30;
            padding: 15px;
            border-radius: 8px;
        }}
        .screenshot img {{
            width: 100%;
            border: 1px solid #3e3e42;
            border-radius: 4px;
        }}
        .screenshot h3 {{
            margin: 0 0 10px 0;
            color: #ce9178;
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
            margin: 20px 0;
        }}
        th {{
            background: #2d2d30;
            text-align: left;
            padding: 10px;
            border: 1px solid #3e3e42;
        }}
        td {{
            padding: 8px;
            border: 1px solid #3e3e42;
        }}
        .metric-value {{
            color: #b5cea8;
            font-family: monospace;
        }}
    </style>
</head>
<body>
    <div class="header">
        <h1>E2E Test Evidence Report</h1>
        <p>{self.test_name}</p>
    </div>

    <h2>Test Metadata</h2>
    <div class="metadata">
        <div class="label">Start Time:</div>
        <div>{self.start_time.strftime('%Y-%m-%d %H:%M:%S')}</div>

        <div class="label">Duration:</div>
        <div>{duration:.2f} seconds</div>

        <div class="label">Screenshots Captured:</div>
        <div>{len(self.screenshots)}</div>

        <div class="label">Metrics Recorded:</div>
        <div>{len(self.metrics)}</div>

        <div class="label">WebSocket Messages:</div>
        <div>{len(self.websocket_log)}</div>

        <div class="label">Evidence Directory:</div>
        <div>{self.test_dir}</div>
    </div>

    <h2>Screenshots</h2>
    <div class="screenshots">
        {screenshots_html}
    </div>

    <h2>Performance Metrics</h2>
    {metrics_html if metrics_html else "<p>No metrics recorded</p>"}

    <h2>Raw Data Files</h2>
    <ul>
        <li><a href="summary.json">summary.json</a> - Test summary metadata</li>
        {'<li><a href="metrics.json">metrics.json</a> - Performance metrics</li>' if self.metrics else ''}
        {'<li><a href="websocket-traffic.json">websocket-traffic.json</a> - WebSocket message log</li>' if self.websocket_log else ''}
        {'<li><a href="test-log.txt">test-log.txt</a> - Test execution log</li>' if self.logs else ''}
    </ul>

    <footer style="margin-top: 40px; padding-top: 20px; border-top: 1px solid #3e3e42; color: #858585;">
        Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}<br>
        MONOTERMINAL Phase 1 E2E Test Suite
    </footer>
</body>
</html>
"""

        report_path.write_text(html_content, encoding="utf-8")
        self.log(f"HTML report generated: {report_path}")
        return report_path


class PerformanceMonitor:
    """
    Monitors and records performance metrics during E2E tests.
    """

    def __init__(self, evidence_collector: EvidenceCollector):
        self.collector = evidence_collector
        self.start_time = time.time()
        self.latency_samples: List[float] = []

    def record_latency(self, operation: str, latency_ms: float):
        """Record operation latency."""
        self.latency_samples.append(latency_ms)
        self.collector.record_metric(
            name=f"latency_{operation}",
            value=latency_ms,
            unit="ms"
        )

    def get_latency_stats(self) -> Dict[str, float]:
        """Calculate latency statistics."""
        if not self.latency_samples:
            return {}

        sorted_samples = sorted(self.latency_samples)
        count = len(sorted_samples)

        return {
            "min": sorted_samples[0],
            "max": sorted_samples[-1],
            "mean": sum(sorted_samples) / count,
            "median": sorted_samples[count // 2],
            "p95": sorted_samples[int(count * 0.95)],
            "p99": sorted_samples[int(count * 0.99)],
        }

    async def measure_async(self, operation: str, coro):
        """Measure and record latency of an async operation."""
        start = time.time()
        result = await coro
        latency_ms = (time.time() - start) * 1000
        self.record_latency(operation, latency_ms)
        return result


class WebSocketTrafficAnalyzer:
    """
    Analyzes WebSocket traffic patterns and protocol efficiency.
    """

    def __init__(self, evidence_collector: EvidenceCollector):
        self.collector = evidence_collector

    def analyze_traffic(self) -> Dict[str, Any]:
        """Analyze recorded WebSocket traffic."""
        log = self.collector.websocket_log

        if not log:
            return {}

        total_sent = sum(
            msg["payload_size"] for msg in log if msg["direction"] == "sent"
        )
        total_received = sum(
            msg["payload_size"] for msg in log if msg["direction"] == "received"
        )

        message_types = {}
        for msg in log:
            msg_type = msg["message_type"]
            message_types[msg_type] = message_types.get(msg_type, 0) + 1

        return {
            "total_messages": len(log),
            "total_sent_bytes": total_sent,
            "total_received_bytes": total_received,
            "total_bytes": total_sent + total_received,
            "message_types": message_types,
            "compression_ratio": self._estimate_compression_ratio(log),
        }

    def _estimate_compression_ratio(self, log: List[Dict]) -> Optional[float]:
        """Estimate compression ratio if compression is used."""
        # Placeholder - would need actual uncompressed size tracking
        return None
