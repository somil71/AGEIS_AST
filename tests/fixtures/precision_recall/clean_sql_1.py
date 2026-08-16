query = "SELECT * FROM users WHERE username = %s"
db.execute(query, (user_input,))