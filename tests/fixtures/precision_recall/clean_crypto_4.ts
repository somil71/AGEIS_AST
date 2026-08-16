import crypto from "crypto";
const hash = crypto.createHash("sha256").update(data).digest("hex");