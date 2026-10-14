@echo off
echo Starting training from book 2...
"<USER_HOME>\AppData\Local\Programs\Python\Python314\python.exe" -u "<AI_FACTORY_ROOT>\tools\train_all_books.py" --resume-from 1
echo Training finished.
pause
