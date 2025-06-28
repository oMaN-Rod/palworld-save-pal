# PowerShell script to generate TypeScript types from Pydantic models
# Requires: pip install pydantic2ts

pydantic2ts .\models.py > .\types.ts
