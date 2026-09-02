# Railway deployment

This directory defines the complete Keith Railway project with the TypeScript
Infrastructure-as-Code interface. Secrets are read from the operator's environment
only while Railway creates a redacted plan; they are never stored in this file.

Use `./keith deploy railway` to preview the plan. Applying it requires both
`--execute` and `KEITH_DEPLOY_APPROVED=YES`.

