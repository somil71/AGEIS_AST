q := "DELETE FROM logs WHERE user_id = $1"
db.Exec(q, req.URL.Query().Get("id"))