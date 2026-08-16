const sql = "UPDATE users SET age = " + age + " WHERE id = " + id;
db.query(sql);