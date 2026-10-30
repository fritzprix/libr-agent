FROM python:3.12-slim-bookworm

WORKDIR /app

# curl: managed mode runs the session in this image, and wake hooks are usually curl.
RUN apt-get update && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*
RUN pip install --no-cache-dir pytest==8.4.1 pytest-json-ctrf==0.3.5

COPY seed/ /app/
RUN chmod +x /app/sms_monitor
