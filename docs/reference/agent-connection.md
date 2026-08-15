# AgentConnection protocol schema

Generated from `keith-protocol` 0.1.0 for protocol 1.0. Do not edit by hand.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "WireMessage",
  "oneOf": [
    {
      "type": "object",
      "properties": {
        "message": {
          "type": "string",
          "const": "client_hello"
        },
        "payload": {
          "$ref": "#/$defs/ClientHello"
        }
      },
      "required": [
        "message",
        "payload"
      ]
    },
    {
      "type": "object",
      "properties": {
        "message": {
          "type": "string",
          "const": "server_hello"
        },
        "payload": {
          "$ref": "#/$defs/ServerHello"
        }
      },
      "required": [
        "message",
        "payload"
      ]
    },
    {
      "type": "object",
      "properties": {
        "message": {
          "type": "string",
          "const": "command"
        },
        "payload": {
          "$ref": "#/$defs/CommandEnvelope"
        }
      },
      "required": [
        "message",
        "payload"
      ]
    },
    {
      "type": "object",
      "properties": {
        "message": {
          "type": "string",
          "const": "command_result"
        },
        "payload": {
          "$ref": "#/$defs/CommandResultEnvelope"
        }
      },
      "required": [
        "message",
        "payload"
      ]
    },
    {
      "type": "object",
      "properties": {
        "message": {
          "type": "string",
          "const": "event"
        },
        "payload": {
          "$ref": "#/$defs/EventEnvelope"
        }
      },
      "required": [
        "message",
        "payload"
      ]
    }
  ],
  "$defs": {
    "ActionProjection": {
      "type": "object",
      "properties": {
        "action_id": {
          "type": "string"
        },
        "created_at": {
          "type": "integer",
          "format": "int64"
        },
        "source": {
          "type": "string"
        },
        "state": {
          "type": "string"
        }
      },
      "required": [
        "action_id",
        "source",
        "state",
        "created_at"
      ]
    },
    "AttachSession": {
      "type": "object",
      "properties": {
        "resume": {
          "anyOf": [
            {
              "$ref": "#/$defs/ResumeCursor"
            },
            {
              "type": "null"
            }
          ]
        },
        "session_id": {
          "type": "string"
        }
      },
      "required": [
        "session_id"
      ]
    },
    "BackgroundControl": {
      "type": "object",
      "properties": {
        "mode": {
          "$ref": "#/$defs/BackgroundMode"
        },
        "pause_until": {
          "type": [
            "integer",
            "null"
          ],
          "format": "int64"
        },
        "profile_id": {
          "type": "string"
        }
      },
      "required": [
        "profile_id",
        "mode"
      ]
    },
    "BackgroundMode": {
      "type": "string",
      "enum": [
        "disabled",
        "suggest",
        "confirm_selected",
        "bounded"
      ]
    },
    "BackgroundProjection": {
      "type": "object",
      "properties": {
        "mode": {
          "$ref": "#/$defs/BackgroundMode"
        },
        "pause_until": {
          "type": [
            "integer",
            "null"
          ],
          "format": "int64"
        },
        "profile_id": {
          "type": "string"
        }
      },
      "required": [
        "profile_id",
        "mode"
      ]
    },
    "BranchRequest": {
      "type": "object",
      "properties": {
        "label": {
          "type": [
            "string",
            "null"
          ]
        },
        "parent_entry_id": {
          "type": "string"
        },
        "session_id": {
          "type": "string"
        }
      },
      "required": [
        "session_id",
        "parent_entry_id"
      ]
    },
    "CancelTarget": {
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "id": {
              "type": "string"
            },
            "target": {
              "type": "string",
              "const": "action"
            }
          },
          "required": [
            "target",
            "id"
          ]
        },
        {
          "type": "object",
          "properties": {
            "id": {
              "type": "string"
            },
            "target": {
              "type": "string",
              "const": "goal"
            }
          },
          "required": [
            "target",
            "id"
          ]
        },
        {
          "type": "object",
          "properties": {
            "id": {
              "type": "string"
            },
            "target": {
              "type": "string",
              "const": "session"
            }
          },
          "required": [
            "target",
            "id"
          ]
        },
        {
          "type": "object",
          "properties": {
            "id": {
              "type": "string"
            },
            "target": {
              "type": "string",
              "const": "child"
            }
          },
          "required": [
            "target",
            "id"
          ]
        }
      ]
    },
    "ChildMessageRequest": {
      "type": "object",
      "properties": {
        "artifact_ids": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "child_id": {
          "type": "string"
        },
        "text": {
          "type": "string"
        }
      },
      "required": [
        "child_id",
        "text",
        "artifact_ids"
      ]
    },
    "ChildProjection": {
      "type": "object",
      "properties": {
        "child_id": {
          "type": "string"
        },
        "objective": {
          "type": "string"
        },
        "session_id": {
          "type": "string"
        },
        "state": {
          "type": "string"
        }
      },
      "required": [
        "child_id",
        "session_id",
        "objective",
        "state"
      ]
    },
    "ChildWorkspaceMode": {
      "type": "string",
      "enum": [
        "read_only_parent",
        "isolated_copy",
        "dedicated_workspace",
        "shared_workspace"
      ]
    },
    "ClientCommand": {
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "list_profiles"
            }
          },
          "required": [
            "command"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "list_sessions"
            },
            "parameters": {
              "$ref": "#/$defs/SessionFilter"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "create_session"
            },
            "parameters": {
              "$ref": "#/$defs/CreateSession"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "attach_session"
            },
            "parameters": {
              "$ref": "#/$defs/AttachSession"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "detach_session"
            },
            "parameters": {
              "type": "object",
              "properties": {
                "session_id": {
                  "type": "string"
                }
              },
              "required": [
                "session_id"
              ]
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "acknowledge_events"
            },
            "parameters": {
              "$ref": "#/$defs/EventAcknowledgement"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "resume_session"
            },
            "parameters": {
              "type": "object",
              "properties": {
                "session_id": {
                  "type": "string"
                }
              },
              "required": [
                "session_id"
              ]
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "branch_session"
            },
            "parameters": {
              "$ref": "#/$defs/BranchRequest"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "select_branch"
            },
            "parameters": {
              "$ref": "#/$defs/SelectBranch"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "submit_prompt"
            },
            "parameters": {
              "$ref": "#/$defs/SubmitPrompt"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "steer"
            },
            "parameters": {
              "$ref": "#/$defs/SteerAction"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "cancel"
            },
            "parameters": {
              "$ref": "#/$defs/CancelTarget"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "select_model"
            },
            "parameters": {
              "$ref": "#/$defs/ModelSelection"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "create_goal"
            },
            "parameters": {
              "$ref": "#/$defs/CreateGoal"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "update_goal"
            },
            "parameters": {
              "$ref": "#/$defs/UpdateGoal"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "list_goals"
            },
            "parameters": {
              "type": "object",
              "properties": {
                "session_id": {
                  "type": "string"
                }
              },
              "required": [
                "session_id"
              ]
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "list_children"
            },
            "parameters": {
              "type": "object",
              "properties": {
                "session_id": {
                  "type": "string"
                }
              },
              "required": [
                "session_id"
              ]
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "create_child"
            },
            "parameters": {
              "$ref": "#/$defs/CreateChild"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "send_child_message"
            },
            "parameters": {
              "$ref": "#/$defs/ChildMessageRequest"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "archive_child"
            },
            "parameters": {
              "type": "object",
              "properties": {
                "child_id": {
                  "type": "string"
                }
              },
              "required": [
                "child_id"
              ]
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "create_schedule"
            },
            "parameters": {
              "$ref": "#/$defs/CreateSchedule"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "update_schedule"
            },
            "parameters": {
              "$ref": "#/$defs/UpdateSchedule"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "delete_schedule"
            },
            "parameters": {
              "type": "object",
              "properties": {
                "job_id": {
                  "type": "string"
                }
              },
              "required": [
                "job_id"
              ]
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "query_memory"
            },
            "parameters": {
              "$ref": "#/$defs/MemoryQuery"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "resolve_confirmation"
            },
            "parameters": {
              "$ref": "#/$defs/ConfirmationResolution"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "export"
            },
            "parameters": {
              "$ref": "#/$defs/ExportRequest"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        },
        {
          "type": "object",
          "properties": {
            "command": {
              "type": "string",
              "const": "set_background_control"
            },
            "parameters": {
              "$ref": "#/$defs/BackgroundControl"
            }
          },
          "required": [
            "command",
            "parameters"
          ]
        }
      ]
    },
    "ClientHello": {
      "type": "object",
      "properties": {
        "client_id": {
          "type": "string"
        },
        "client_name": {
          "type": "string"
        },
        "client_version": {
          "type": "string"
        },
        "protocol": {
          "$ref": "#/$defs/ProtocolVersion"
        },
        "resume": {
          "anyOf": [
            {
              "$ref": "#/$defs/ResumeCursor"
            },
            {
              "type": "null"
            }
          ]
        },
        "supported_features": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Feature"
          },
          "uniqueItems": true
        }
      },
      "required": [
        "protocol",
        "client_id",
        "client_name",
        "client_version",
        "supported_features"
      ]
    },
    "CommandEnvelope": {
      "type": "object",
      "properties": {
        "client_id": {
          "type": "string"
        },
        "command": {
          "$ref": "#/$defs/ClientCommand"
        },
        "command_id": {
          "type": "string"
        },
        "protocol": {
          "$ref": "#/$defs/ProtocolVersion"
        },
        "sent_at": {
          "type": "integer",
          "format": "int64"
        },
        "session_id": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "protocol",
        "command_id",
        "client_id",
        "sent_at",
        "command"
      ]
    },
    "CommandError": {
      "type": "object",
      "properties": {
        "error": {
          "$ref": "#/$defs/CommonError"
        },
        "unsupported_feature": {
          "anyOf": [
            {
              "$ref": "#/$defs/Feature"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "error"
      ]
    },
    "CommandResult": {
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "payload": {
              "type": "object",
              "properties": {
                "action_id": {
                  "type": [
                    "string",
                    "null"
                  ]
                }
              }
            },
            "status": {
              "type": "string",
              "const": "accepted"
            }
          },
          "required": [
            "status",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "payload": {
              "$ref": "#/$defs/ResponsePayload"
            },
            "status": {
              "type": "string",
              "const": "data"
            }
          },
          "required": [
            "status",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "payload": {
              "$ref": "#/$defs/CommandError"
            },
            "status": {
              "type": "string",
              "const": "rejected"
            }
          },
          "required": [
            "status",
            "payload"
          ]
        }
      ]
    },
    "CommandResultEnvelope": {
      "type": "object",
      "properties": {
        "command_id": {
          "type": "string"
        },
        "completed_at": {
          "type": "integer",
          "format": "int64"
        },
        "protocol": {
          "$ref": "#/$defs/ProtocolVersion"
        },
        "result": {
          "$ref": "#/$defs/CommandResult"
        }
      },
      "required": [
        "protocol",
        "command_id",
        "completed_at",
        "result"
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
    "ConfirmationDecision": {
      "type": "string",
      "enum": [
        "allow_once",
        "allow_for_session",
        "deny"
      ]
    },
    "ConfirmationProjection": {
      "type": "object",
      "properties": {
        "confirmation_id": {
          "type": "string"
        },
        "summary": {
          "type": "string"
        }
      },
      "required": [
        "confirmation_id",
        "summary"
      ]
    },
    "ConfirmationResolution": {
      "type": "object",
      "properties": {
        "confirmation_id": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ConfirmationDecision"
        }
      },
      "required": [
        "confirmation_id",
        "decision"
      ]
    },
    "CreateChild": {
      "type": "object",
      "properties": {
        "limits": {
          "$ref": "#/$defs/GoalLimits"
        },
        "objective": {
          "type": "string"
        },
        "parent_session_id": {
          "type": "string"
        },
        "workspace_mode": {
          "$ref": "#/$defs/ChildWorkspaceMode"
        }
      },
      "required": [
        "parent_session_id",
        "objective",
        "workspace_mode",
        "limits"
      ]
    },
    "CreateGoal": {
      "type": "object",
      "properties": {
        "limits": {
          "$ref": "#/$defs/GoalLimits"
        },
        "objective": {
          "type": "string"
        },
        "session_id": {
          "type": "string"
        }
      },
      "required": [
        "session_id",
        "objective",
        "limits"
      ]
    },
    "CreateSchedule": {
      "type": "object",
      "properties": {
        "expression": {
          "$ref": "#/$defs/ScheduleExpression"
        },
        "profile_id": {
          "type": "string"
        },
        "prompt": {
          "type": "string"
        },
        "reply_route": {
          "anyOf": [
            {
              "$ref": "#/$defs/ReplyRoute"
            },
            {
              "type": "null"
            }
          ]
        },
        "session_id": {
          "type": [
            "string",
            "null"
          ]
        },
        "time_zone": {
          "type": "string"
        }
      },
      "required": [
        "profile_id",
        "expression",
        "time_zone",
        "prompt"
      ]
    },
    "CreateSession": {
      "type": "object",
      "properties": {
        "profile_id": {
          "type": "string"
        },
        "title": {
          "type": [
            "string",
            "null"
          ]
        },
        "workspace_id": {
          "type": "string"
        }
      },
      "required": [
        "profile_id",
        "workspace_id"
      ]
    },
    "DaemonEvent": {
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "snapshot"
            },
            "payload": {
              "$ref": "#/$defs/SessionSnapshot"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "command_accepted"
            },
            "payload": {
              "type": "object",
              "properties": {
                "command_id": {
                  "type": "string"
                }
              },
              "required": [
                "command_id"
              ]
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "command_rejected"
            },
            "payload": {
              "$ref": "#/$defs/CommandError"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "session_changed"
            },
            "payload": {
              "$ref": "#/$defs/SessionSummary"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "action_queued"
            },
            "payload": {
              "$ref": "#/$defs/ActionProjection"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "action_started"
            },
            "payload": {
              "$ref": "#/$defs/ActionProjection"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "action_finished"
            },
            "payload": {
              "$ref": "#/$defs/ActionProjection"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "assistant_delta"
            },
            "payload": {
              "type": "object",
              "properties": {
                "message_id": {
                  "type": "string"
                },
                "text": {
                  "type": "string"
                }
              },
              "required": [
                "message_id",
                "text"
              ]
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "message_committed"
            },
            "payload": {
              "$ref": "#/$defs/MessageProjection"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "goal_changed"
            },
            "payload": {
              "$ref": "#/$defs/GoalProjection"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "child_changed"
            },
            "payload": {
              "$ref": "#/$defs/ChildProjection"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "schedule_changed"
            },
            "payload": {
              "$ref": "#/$defs/ScheduleProjection"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "tool_changed"
            },
            "payload": {
              "$ref": "#/$defs/ToolProjection"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "wait_changed"
            },
            "payload": {
              "$ref": "#/$defs/WaitProjection"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "delivery_changed"
            },
            "payload": {
              "$ref": "#/$defs/DeliveryProjection"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "confirmation_requested"
            },
            "payload": {
              "type": "object",
              "properties": {
                "confirmation_id": {
                  "type": "string"
                },
                "summary": {
                  "type": "string"
                }
              },
              "required": [
                "confirmation_id",
                "summary"
              ]
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "confirmation_resolved"
            },
            "payload": {
              "type": "object",
              "properties": {
                "confirmation_id": {
                  "type": "string"
                }
              },
              "required": [
                "confirmation_id"
              ]
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "warning"
            },
            "payload": {
              "$ref": "#/$defs/CommonError"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        },
        {
          "type": "object",
          "properties": {
            "event": {
              "type": "string",
              "const": "error"
            },
            "payload": {
              "$ref": "#/$defs/CommonError"
            }
          },
          "required": [
            "event",
            "payload"
          ]
        }
      ]
    },
    "DeliveryPolicy": {
      "type": "string",
      "enum": [
        "immediate",
        "next_turn_boundary",
        "when_idle"
      ]
    },
    "DeliveryProjection": {
      "type": "object",
      "properties": {
        "delivery_id": {
          "type": "string"
        },
        "state": {
          "type": "string"
        },
        "terminal": {
          "type": "boolean"
        }
      },
      "required": [
        "delivery_id",
        "state",
        "terminal"
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
    "EventAcknowledgement": {
      "type": "object",
      "properties": {
        "generation": {
          "type": "integer",
          "format": "uint64",
          "minimum": 0
        },
        "root_tree_id": {
          "type": "string"
        },
        "through_sequence": {
          "type": "integer",
          "format": "uint64",
          "minimum": 0
        }
      },
      "required": [
        "root_tree_id",
        "generation",
        "through_sequence"
      ]
    },
    "EventEnvelope": {
      "type": "object",
      "properties": {
        "event": {
          "$ref": "#/$defs/DaemonEvent"
        },
        "generation": {
          "type": "integer",
          "format": "uint64",
          "minimum": 0
        },
        "occurred_at": {
          "type": "integer",
          "format": "int64"
        },
        "protocol": {
          "$ref": "#/$defs/ProtocolVersion"
        },
        "root_tree_id": {
          "type": "string"
        },
        "sequence": {
          "type": "integer",
          "format": "uint64",
          "minimum": 0
        }
      },
      "required": [
        "protocol",
        "root_tree_id",
        "generation",
        "sequence",
        "occurred_at",
        "event"
      ]
    },
    "ExportFormat": {
      "type": "string",
      "enum": [
        "json_lines",
        "markdown",
        "portable_bundle"
      ]
    },
    "ExportProjection": {
      "type": "object",
      "properties": {
        "artifact_id": {
          "type": "string"
        },
        "byte_length": {
          "type": "integer",
          "format": "uint64",
          "minimum": 0
        },
        "media_type": {
          "type": "string"
        }
      },
      "required": [
        "artifact_id",
        "media_type",
        "byte_length"
      ]
    },
    "ExportRequest": {
      "type": "object",
      "properties": {
        "format": {
          "$ref": "#/$defs/ExportFormat"
        },
        "include_artifacts": {
          "type": "boolean"
        },
        "session_id": {
          "type": "string"
        }
      },
      "required": [
        "session_id",
        "format",
        "include_artifacts"
      ]
    },
    "Feature": {
      "type": "string",
      "enum": [
        "session_lifecycle",
        "branching",
        "steering",
        "goals",
        "children",
        "schedules",
        "memory_queries",
        "confirmations",
        "export",
        "background_controls",
        "replay",
        "snapshots",
        "framed_json",
        "local_binary",
        "stdio",
        "web_socket"
      ]
    },
    "GoalLimits": {
      "type": "object",
      "properties": {
        "deadline": {
          "type": [
            "integer",
            "null"
          ],
          "format": "int64"
        },
        "max_tokens": {
          "type": [
            "integer",
            "null"
          ],
          "format": "uint64",
          "minimum": 0
        },
        "max_turns": {
          "type": [
            "integer",
            "null"
          ],
          "format": "uint32",
          "minimum": 0
        }
      }
    },
    "GoalProjection": {
      "type": "object",
      "properties": {
        "goal_id": {
          "type": "string"
        },
        "objective": {
          "type": "string"
        },
        "state": {
          "$ref": "#/$defs/GoalState"
        }
      },
      "required": [
        "goal_id",
        "objective",
        "state"
      ]
    },
    "GoalState": {
      "type": "string",
      "enum": [
        "draft",
        "ready",
        "running",
        "waiting",
        "reviewing",
        "paused",
        "blocked",
        "complete",
        "failed",
        "cancelled"
      ]
    },
    "MemoryQuery": {
      "type": "object",
      "properties": {
        "limit": {
          "type": "integer",
          "format": "uint",
          "minimum": 0
        },
        "profile_id": {
          "type": "string"
        },
        "query": {
          "type": "string"
        }
      },
      "required": [
        "profile_id",
        "query",
        "limit"
      ]
    },
    "MemoryResult": {
      "type": "object",
      "properties": {
        "excerpt": {
          "type": "string"
        },
        "score_micros": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "source": {
          "type": "string"
        }
      },
      "required": [
        "source",
        "excerpt",
        "score_micros"
      ]
    },
    "MessageProjection": {
      "type": "object",
      "properties": {
        "committed": {
          "type": "boolean"
        },
        "message_id": {
          "type": "string"
        },
        "role": {
          "$ref": "#/$defs/MessageRole"
        },
        "text": {
          "type": "string"
        }
      },
      "required": [
        "message_id",
        "role",
        "text",
        "committed"
      ]
    },
    "MessageRole": {
      "type": "string",
      "enum": [
        "user",
        "assistant",
        "tool",
        "system"
      ]
    },
    "ModelSelection": {
      "type": "object",
      "properties": {
        "model": {
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "session_id": {
          "type": "string"
        }
      },
      "required": [
        "session_id",
        "provider",
        "model"
      ]
    },
    "ProfileSummary": {
      "type": "object",
      "properties": {
        "display_name": {
          "type": "string"
        },
        "enabled": {
          "type": "boolean"
        },
        "id": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "display_name",
        "enabled"
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
    "ReplyRoute": {
      "type": "object",
      "properties": {
        "channel": {
          "type": "string"
        },
        "conversation": {
          "type": "string"
        },
        "thread": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "channel",
        "conversation"
      ]
    },
    "ResponsePayload": {
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "kind": {
              "type": "string",
              "const": "profiles"
            },
            "value": {
              "type": "array",
              "items": {
                "$ref": "#/$defs/ProfileSummary"
              }
            }
          },
          "required": [
            "kind",
            "value"
          ]
        },
        {
          "type": "object",
          "properties": {
            "kind": {
              "type": "string",
              "const": "sessions"
            },
            "value": {
              "type": "array",
              "items": {
                "$ref": "#/$defs/SessionSummary"
              }
            }
          },
          "required": [
            "kind",
            "value"
          ]
        },
        {
          "type": "object",
          "properties": {
            "kind": {
              "type": "string",
              "const": "snapshot"
            },
            "value": {
              "$ref": "#/$defs/SessionSnapshot"
            }
          },
          "required": [
            "kind",
            "value"
          ]
        },
        {
          "type": "object",
          "properties": {
            "kind": {
              "type": "string",
              "const": "goal"
            },
            "value": {
              "$ref": "#/$defs/GoalProjection"
            }
          },
          "required": [
            "kind",
            "value"
          ]
        },
        {
          "type": "object",
          "properties": {
            "kind": {
              "type": "string",
              "const": "child"
            },
            "value": {
              "$ref": "#/$defs/ChildProjection"
            }
          },
          "required": [
            "kind",
            "value"
          ]
        },
        {
          "type": "object",
          "properties": {
            "kind": {
              "type": "string",
              "const": "schedule"
            },
            "value": {
              "$ref": "#/$defs/ScheduleProjection"
            }
          },
          "required": [
            "kind",
            "value"
          ]
        },
        {
          "type": "object",
          "properties": {
            "kind": {
              "type": "string",
              "const": "memory"
            },
            "value": {
              "type": "array",
              "items": {
                "$ref": "#/$defs/MemoryResult"
              }
            }
          },
          "required": [
            "kind",
            "value"
          ]
        },
        {
          "type": "object",
          "properties": {
            "kind": {
              "type": "string",
              "const": "export"
            },
            "value": {
              "$ref": "#/$defs/ExportProjection"
            }
          },
          "required": [
            "kind",
            "value"
          ]
        },
        {
          "type": "object",
          "properties": {
            "kind": {
              "type": "string",
              "const": "background"
            },
            "value": {
              "$ref": "#/$defs/BackgroundProjection"
            }
          },
          "required": [
            "kind",
            "value"
          ]
        }
      ]
    },
    "ResumeCursor": {
      "type": "object",
      "properties": {
        "generation": {
          "type": "integer",
          "format": "uint64",
          "minimum": 0
        },
        "last_sequence": {
          "type": "integer",
          "format": "uint64",
          "minimum": 0
        },
        "root_tree_id": {
          "type": "string"
        }
      },
      "required": [
        "root_tree_id",
        "generation",
        "last_sequence"
      ]
    },
    "ResumeMode": {
      "type": "string",
      "enum": [
        "fresh",
        "delta",
        "snapshot_then_delta",
        "incompatible"
      ]
    },
    "ScheduleExpression": {
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "kind": {
              "type": "string",
              "const": "once"
            },
            "value": {
              "type": "integer",
              "format": "int64"
            }
          },
          "required": [
            "kind",
            "value"
          ]
        },
        {
          "type": "object",
          "properties": {
            "kind": {
              "type": "string",
              "const": "interval_seconds"
            },
            "value": {
              "type": "integer",
              "format": "uint64",
              "minimum": 0
            }
          },
          "required": [
            "kind",
            "value"
          ]
        },
        {
          "type": "object",
          "properties": {
            "kind": {
              "type": "string",
              "const": "calendar"
            },
            "value": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "value"
          ]
        }
      ]
    },
    "ScheduleProjection": {
      "type": "object",
      "properties": {
        "expression": {
          "$ref": "#/$defs/ScheduleExpression"
        },
        "job_id": {
          "type": "string"
        },
        "next_run": {
          "type": [
            "integer",
            "null"
          ],
          "format": "int64"
        },
        "paused": {
          "type": "boolean"
        }
      },
      "required": [
        "job_id",
        "expression",
        "paused"
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
    "SelectBranch": {
      "type": "object",
      "properties": {
        "leaf_entry_id": {
          "type": "string"
        },
        "session_id": {
          "type": "string"
        }
      },
      "required": [
        "session_id",
        "leaf_entry_id"
      ]
    },
    "ServerHello": {
      "type": "object",
      "properties": {
        "current_generation": {
          "type": [
            "integer",
            "null"
          ],
          "format": "uint64",
          "minimum": 0
        },
        "protocol": {
          "$ref": "#/$defs/ProtocolVersion"
        },
        "resume_mode": {
          "$ref": "#/$defs/ResumeMode"
        },
        "server_instance_id": {
          "type": "string"
        },
        "supported_features": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Feature"
          },
          "uniqueItems": true
        }
      },
      "required": [
        "protocol",
        "server_instance_id",
        "supported_features",
        "resume_mode"
      ]
    },
    "SessionFilter": {
      "type": "object",
      "properties": {
        "include_archived": {
          "type": "boolean"
        },
        "profile_id": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "include_archived"
      ]
    },
    "SessionSnapshot": {
      "type": "object",
      "properties": {
        "active_action": {
          "anyOf": [
            {
              "$ref": "#/$defs/ActionProjection"
            },
            {
              "type": "null"
            }
          ]
        },
        "children": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/ChildProjection"
          }
        },
        "confirmations": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/ConfirmationProjection"
          }
        },
        "deliveries": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/DeliveryProjection"
          }
        },
        "generation": {
          "type": "integer",
          "format": "uint64",
          "minimum": 0
        },
        "goals": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/GoalProjection"
          }
        },
        "messages": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/MessageProjection"
          }
        },
        "revision": {
          "type": "integer",
          "format": "uint64",
          "minimum": 0
        },
        "schedules": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/ScheduleProjection"
          }
        },
        "session": {
          "$ref": "#/$defs/SessionSummary"
        },
        "through_sequence": {
          "type": "integer",
          "format": "uint64",
          "minimum": 0
        },
        "tools": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/ToolProjection"
          }
        },
        "waits": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/WaitProjection"
          }
        }
      },
      "required": [
        "session",
        "generation",
        "through_sequence",
        "messages",
        "goals",
        "children",
        "schedules",
        "tools",
        "confirmations",
        "waits",
        "deliveries",
        "revision"
      ]
    },
    "SessionState": {
      "type": "string",
      "enum": [
        "dormant",
        "ready",
        "running",
        "waiting_tool",
        "waiting_child",
        "waiting_external",
        "compacting",
        "paused",
        "failed",
        "archived"
      ]
    },
    "SessionSummary": {
      "type": "object",
      "properties": {
        "profile_id": {
          "type": "string"
        },
        "root_tree_id": {
          "type": "string"
        },
        "session_id": {
          "type": "string"
        },
        "state": {
          "$ref": "#/$defs/SessionState"
        },
        "title": {
          "type": [
            "string",
            "null"
          ]
        },
        "updated_at": {
          "type": "integer",
          "format": "int64"
        }
      },
      "required": [
        "session_id",
        "root_tree_id",
        "profile_id",
        "state",
        "updated_at"
      ]
    },
    "SteerAction": {
      "type": "object",
      "properties": {
        "delivery": {
          "$ref": "#/$defs/DeliveryPolicy"
        },
        "session_id": {
          "type": "string"
        },
        "text": {
          "type": "string"
        }
      },
      "required": [
        "session_id",
        "text",
        "delivery"
      ]
    },
    "SubmitPrompt": {
      "type": "object",
      "properties": {
        "delivery": {
          "$ref": "#/$defs/DeliveryPolicy"
        },
        "reply_route": {
          "anyOf": [
            {
              "$ref": "#/$defs/ReplyRoute"
            },
            {
              "type": "null"
            }
          ]
        },
        "session_id": {
          "type": "string"
        },
        "text": {
          "type": "string"
        }
      },
      "required": [
        "session_id",
        "text",
        "delivery"
      ]
    },
    "ToolProjection": {
      "type": "object",
      "properties": {
        "state": {
          "type": "string"
        },
        "terminal": {
          "type": "boolean"
        },
        "tool_call_id": {
          "type": "string"
        }
      },
      "required": [
        "tool_call_id",
        "state",
        "terminal"
      ]
    },
    "UpdateGoal": {
      "type": "object",
      "properties": {
        "goal_id": {
          "type": "string"
        },
        "limits": {
          "anyOf": [
            {
              "$ref": "#/$defs/GoalLimits"
            },
            {
              "type": "null"
            }
          ]
        },
        "objective": {
          "type": [
            "string",
            "null"
          ]
        },
        "state": {
          "anyOf": [
            {
              "$ref": "#/$defs/GoalState"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "goal_id"
      ]
    },
    "UpdateSchedule": {
      "type": "object",
      "properties": {
        "expression": {
          "anyOf": [
            {
              "$ref": "#/$defs/ScheduleExpression"
            },
            {
              "type": "null"
            }
          ]
        },
        "job_id": {
          "type": "string"
        },
        "paused": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "prompt": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "job_id"
      ]
    },
    "WaitProjection": {
      "type": "object",
      "properties": {
        "state": {
          "type": "string"
        },
        "terminal": {
          "type": "boolean"
        },
        "wait_id": {
          "type": "string"
        }
      },
      "required": [
        "wait_id",
        "state",
        "terminal"
      ]
    }
  }
}
```
