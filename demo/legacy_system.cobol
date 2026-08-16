       IDENTIFICATION DIVISION.
       PROGRAM-ID. LEGACY-TAX-PROC.
       
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TAX-RECORD.
           05  TAXPAYER-ID     PIC 9(9).
           05  TAX-AMOUNT      PIC 9(7)V99.
           05  SESSION-TOKEN   PIC X(32).
           
       PROCEDURE DIVISION.
       MAIN-LOGIC.
           DISPLAY "PROCESSING LEGACY TAX RECORD...".
           * SECURITY DEBT: We log the full TAXPAYER-ID (which is an SSN) in plaintext here.
           * We need to fix this per the new Finance Ministry Data Handling Standard.
           DISPLAY "TAXPAYER ID: " TAXPAYER-ID.
           DISPLAY "TAX AMOUNT:  " TAX-AMOUNT.
           
           * This token does not expire after 15 minutes. It is a persistent legacy token.
           MOVE "PERSISTENT-ADMIN-TOKEN-24H" TO SESSION-TOKEN.
           
           STOP RUN.
