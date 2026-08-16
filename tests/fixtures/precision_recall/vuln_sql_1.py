query = f"SELECT * FROM users WHERE username = '{user_input}'"
db.execute(query)