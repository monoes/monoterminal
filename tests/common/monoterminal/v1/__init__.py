"""MONOTERMINAL v1 protocol messages."""

from .messages_pb2 import (
    # Core envelope
    Envelope,

    # Request/Response messages
    AttachRequest,
    AttachResponse,
    InputData,
    OutputData,
    ResizeRequest,
    DetachRequest,
    ErrorResponse,
    DashboardRequest,
    DashboardResponse,

    # Supporting types
    SessionMetadata,
    Line,

    # Monomind integration
    HealthCheckRequest,
    HealthCheckResponse,
    HealthIssue,
    UpgradeRequest,
    UpgradeResponse,
    DetectionRequest,
    DetectionResponse,
    MonitoringData,
    RunSummary,

    # Enums
    CompressionType,
    ErrorCode,
    IssueSeverity,
)

__all__ = [
    # Core
    "Envelope",

    # Messages
    "AttachRequest",
    "AttachResponse",
    "InputData",
    "OutputData",
    "ResizeRequest",
    "DetachRequest",
    "ErrorResponse",
    "DashboardRequest",
    "DashboardResponse",

    # Types
    "SessionMetadata",
    "Line",

    # Monomind
    "HealthCheckRequest",
    "HealthCheckResponse",
    "HealthIssue",
    "UpgradeRequest",
    "UpgradeResponse",
    "DetectionRequest",
    "DetectionResponse",
    "MonitoringData",
    "RunSummary",

    # Enums
    "CompressionType",
    "ErrorCode",
    "IssueSeverity",
]
