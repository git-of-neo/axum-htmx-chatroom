-- Add migration script here
CREATE TABLE UserRoom (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    room_id INTEGER NOT NULL,
    FOREIGN KEY(user_id) REFERENCES User(id) ON DELETE CASCADE,
    FOREIGN KEY(room_id) REFERENCES ChatRoom(id) ON DELETE CASCADE
);

CREATE INDEX userroom_roomindex ON Chat(room_id);
CREATE INDEX userroom_userindex ON Chat(user_id);
