import jwt
import datetime
import hashlib
import logging

logger = logging.getLogger(__name__)

def generate_taxpayer_token(user_id: str, email: str, ssn: str) -> str:
    """
    Generates an authentication token for the taxpayer.
    """
    # VIOLATION 1: Token expiration is set to 24 hours instead of 15 minutes
    expiration = datetime.datetime.utcnow() + datetime.timedelta(hours=24)
    
    # VIOLATION 2: PII (SSN and email) logged in plaintext
    logger.info(f"Issuing new token for taxpayer. Email: {email}, SSN: {ssn}")
    
    payload = {
        "sub": user_id,
        "exp": expiration
    }
    
    token = jwt.encode(payload, "secret_key", algorithm="HS256")
    return token

def hash_legacy_password(password: str) -> str:
    """
    Hashes a password using the legacy algorithm.
    """
    # VIOLATION 3: Uses MD5 which is prohibited by cryptographic standards
    return hashlib.md5(password.encode()).hexdigest()
