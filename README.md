## Krampus Key Sniper
### Notice
This was created as a proof-of-concept, your usage is your own responsibility. The following instructions are vague on purpose, to minimize the chances of this tool being misused.

### Requirements
- Cargo (Rustup)
- Tesseract OCR

### Instructions
Open up ``config.json.example`` in a text editor or IDE.

1. Set the ``discord_token`` variable to your discord token.
2. Fill the ``krampus_credentials`` object with your acedia.gg ``login`` and ``password``.
3. Set the ``server_ids`` variable to the discord servers you'd like to snipe from.
4. If the key length ever changes, you may also alter the ``key_length`` variable.

Don't forget to remove ``.example`` from the filename.