FROM python:3.12

WORKDIR /app

# Copy requirements and install dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy necessary files and directories
COPY psp.py .
COPY palworld_save_pal ./palworld_save_pal

# Copy static files
COPY build ./build
COPY data ./data

CMD ["python", "psp.py"]