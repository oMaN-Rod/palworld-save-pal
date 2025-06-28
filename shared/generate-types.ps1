# PowerShell script to generate TypeScript types from Pydantic models
# Requires: pip install pydantic2ts

pydantic2ts --module shared.models --output shared/types.ts
