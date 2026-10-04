FROM python:3.12-slim
WORKDIR /adapter
COPY requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt
COPY demo_adapter ./demo_adapter
COPY import_demo.py ./
ENV PYTHONDONTWRITEBYTECODE=1
ENTRYPOINT ["python", "-B", "import_demo.py"]
