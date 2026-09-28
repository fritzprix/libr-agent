@echo off
REM Stub MediaAssist run for Windows — replace with real ASR/OCR/VLM.
REM Reads one JSON object from stdin; writes one JSON object to stdout.
REM -X utf8 avoids UnicodeDecodeError under isolated OEM/ANSI code pages.
setlocal EnableExtensions
python -X utf8 -c "import json,sys; from pathlib import Path; req=json.load(sys.stdin); modality=str(req.get('modality','')).lower(); path=req.get('path'); max_chars=int(req.get('maxOutputChars') or 8000); ok=bool(path and Path(path).is_file()); text=(f'[stub] modality={modality} path={path}. Replace templates/run.cmd with a real local ASR/OCR/VLM implementation.')[:max_chars] if ok else ''; json.dump({'ok': ok, 'modality': modality, 'text': text, 'notes': 'stub', **({} if ok else {'error':'missing_path','message':f'media path missing or not a file: {path!r}'})}, sys.stdout)"
exit /b %ERRORLEVEL%
