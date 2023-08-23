-- Add migration script here
CREATE TABLE ChatRoom (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL
);

CREATE TABLE Chat(
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER, 
    room_id INTEGER NOT NULL,
    message TEXT DEFAULT "" NOT NULL, 
    time_created DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL,
    FOREIGN KEY(user_id) REFERENCES User(id) ON DELETE SET NULL,
    FOREIGN KEY(room_id) REFERENCES ChatRoom(id) ON DELETE CASCADE
);

CREATE INDEX roomindex ON Chat(room_id);
