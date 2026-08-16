const sql = "UPDATE users SET age = ? WHERE id = ?";
db.query(sql, [age, id]);