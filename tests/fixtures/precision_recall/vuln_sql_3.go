q := "DELETE FROM logs WHERE user_id = " + req.URL.Query().Get("id")
db.Exec(q)