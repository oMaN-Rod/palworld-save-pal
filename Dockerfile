FROM python:3.12 AS builder

# Clone the temp palworld-save-tools repo until it gets updated
RUN git clone https://github.com/oMaN-Rod/palworld-save-tools.git

FROM oven/bun AS ui_builder

ARG PUBLIC_WS_URL=127.0.0.1:5174/ws

COPY . /app
WORKDIR /app/ui
RUN echo "PUBLIC_WS_URL=${PUBLIC_WS_URL}" >.env; \
    echo "PUBLIC_DESKTOP_MODE=false" >>.env; \
    bun install; \
    bun run build

FROM python:3.12

WORKDIR /app

# Copy requirements and install dependencies
COPY requirements-docker.txt .
RUN pip install --no-cache-dir -r requirements-docker.txt

# Copy necessary files and directories
COPY psp.py .
COPY palworld_save_pal ./palworld_save_pal
COPY --from=ui_builder /app/ui_build ./ui
COPY data ./data

CMD ["python", "psp.py"]