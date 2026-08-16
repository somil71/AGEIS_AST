import crypto from "crypto";
const hash = crypto.createHash("md5").update(data).digest("hex");