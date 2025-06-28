# Script to generate TypeScript types from Pydantic models
# Requires: pip install pydantic2ts

import os
os.system("pydantic2ts --input shared/models.py --output shared/types.ts")
print("TypeScript types generated in shared/types.ts")
