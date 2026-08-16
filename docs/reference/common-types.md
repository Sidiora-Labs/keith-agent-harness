# Keith common type schema

Generated from `keith-agent-types` 0.1.0. Do not edit by hand.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "CommonTypesSchema",
  "type": "object",
  "properties": {
    "action_id": {
      "type": "string"
    },
    "artifact_id": {
      "type": "string"
    },
    "child_id": {
      "type": "string"
    },
    "client_id": {
      "type": "string"
    },
    "command_id": {
      "type": "string"
    },
    "commitment_id": {
      "type": "string"
    },
    "delivery_id": {
      "type": "string"
    },
    "entry_id": {
      "type": "string"
    },
    "fixture": {
      "$ref": "#/$defs/CommonCompatibilityFixture"
    },
    "generation": {
      "type": "integer",
      "format": "uint64",
      "minimum": 0
    },
    "goal_id": {
      "type": "string"
    },
    "job_id": {
      "type": "string"
    },
    "message_id": {
      "type": "string"
    },
    "process_instance_id": {
      "type": "string"
    },
    "profile_id": {
      "type": "string"
    },
    "revision": {
      "type": "integer",
      "format": "uint64",
      "minimum": 0
    },
    "root_tree_id": {
      "type": "string"
    },
    "sequence": {
      "type": "integer",
      "format": "uint64",
      "minimum": 0
    },
    "tool_call_id": {
      "type": "string"
    },
    "turn_id": {
      "type": "string"
    },
    "worker_id": {
      "type": "string"
    },
    "workspace_id": {
      "type": "string"
    }
  },
  "required": [
    "fixture",
    "action_id",
    "artifact_id",
    "child_id",
    "client_id",
    "command_id",
    "commitment_id",
    "delivery_id",
    "entry_id",
    "goal_id",
    "job_id",
    "message_id",
    "process_instance_id",
    "profile_id",
    "root_tree_id",
    "tool_call_id",
    "turn_id",
    "worker_id",
    "workspace_id",
    "generation",
    "revision",
    "sequence"
  ],
  "$defs": {
    "CommonCompatibilityFixture": {
      "type": "object",
      "properties": {
        "created_at": {
          "$ref": "#/$defs/ZonedTimestamp"
        },
        "error": {
          "$ref": "#/$defs/CommonError"
        },
        "protocol": {
          "$ref": "#/$defs/ProtocolVersion"
        },
        "schema": {
          "$ref": "#/$defs/SchemaVersion"
        },
        "session_id": {
          "type": "string"
        }
      },
      "additionalProperties": false,
      "required": [
        "schema",
        "protocol",
        "session_id",
        "created_at",
        "error"
      ]
    },
    "CommonError": {
      "type": "object",
      "properties": {
        "code": {
          "$ref": "#/$defs/ErrorCode"
        },
        "message": {
          "type": "string"
        },
        "retryable": {
          "type": "boolean"
        },
        "version": {
          "$ref": "#/$defs/SchemaVersion"
        }
      },
      "additionalProperties": false,
      "required": [
        "version",
        "code",
        "message",
        "retryable"
      ]
    },
    "ErrorCode": {
      "type": "string",
      "enum": [
        "invalid_input",
        "not_found",
        "conflict",
        "unauthorized",
        "forbidden",
        "unsupported_version",
        "unavailable",
        "resource_exhausted",
        "deadline_exceeded",
        "cancelled",
        "corrupt_state",
        "internal"
      ]
    },
    "ProtocolVersion": {
      "type": "object",
      "properties": {
        "major": {
          "type": "integer",
          "format": "uint16",
          "maximum": 65535,
          "minimum": 0
        },
        "minor": {
          "type": "integer",
          "format": "uint16",
          "maximum": 65535,
          "minimum": 0
        }
      },
      "additionalProperties": false,
      "required": [
        "major",
        "minor"
      ]
    },
    "SchemaVersion": {
      "type": "object",
      "properties": {
        "major": {
          "type": "integer",
          "format": "uint16",
          "maximum": 65535,
          "minimum": 0
        },
        "minor": {
          "type": "integer",
          "format": "uint16",
          "maximum": 65535,
          "minimum": 0
        }
      },
      "additionalProperties": false,
      "required": [
        "major",
        "minor"
      ]
    },
    "ZonedTimestamp": {
      "type": "object",
      "properties": {
        "instant": {
          "type": "integer",
          "format": "int64"
        },
        "time_zone": {
          "type": "string"
        }
      },
      "additionalProperties": false,
      "required": [
        "instant",
        "time_zone"
      ]
    }
  }
}
```
