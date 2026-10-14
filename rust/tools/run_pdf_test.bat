@echo off
python -u <AI_FACTORY_ROOT>\tools\train_pdf.py "<USER_HOME>\OneDrive\Desktop\Workplace_Software_and_Skills_-_WEB_IlfJtcP.pdf" --dry-run --start-page 30 --end-page 35 --model mistral:7b > <AI_FACTORY_ROOT>\tools\pdf_test_output.txt 2>&1
echo DONE >> <AI_FACTORY_ROOT>\tools\pdf_test_output.txt
