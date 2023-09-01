use axum::{
    extract::{Multipart, State},
    Extension,
};

use std::{
    sync::Arc,
    {io, path},
};

use askama::Template;
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

use crate::manager::{chat_manager::ChatManager, ChatRoom, User};
use crate::AppState;
use crate::IMAGE_DIR;

#[derive(Template)]
#[template(path = "new_room_results.html")]
pub struct NewRoomResultsTemplate {
    room: Option<ChatRoom>,
}

struct ChatRoomBuilder {
    name: Option<String>,
    image_path: Option<String>,
}

impl ChatRoomBuilder {
    fn new() -> Self {
        Self {
            name: None,
            image_path: None,
        }
    }

    fn set_name(&mut self, name: String) {
        self.name = Some(name);
    }

    fn set_image_path(&mut self, image_path: String) {
        self.image_path = Some(image_path);
    }
}

pub async fn try_new_room(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    mut multipart: Multipart,
) -> NewRoomResultsTemplate {
    let mut builder = ChatRoomBuilder::new();
    while let Some(mut field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        if name == "name" {
            builder.set_name(field.text().await.unwrap())
        } else if name == "image" {
            let file_format = field
                .file_name()
                .unwrap()
                .split('.')
                .last()
                .expect("file name should have a file extension");
            let image_path = format!("{}.{}", Uuid::new_v4(), file_format);

            builder.set_image_path(image_path.clone());

            let mut file_path = path::PathBuf::from(IMAGE_DIR);
            file_path.push(image_path);

            let mut file = match File::create(&file_path).await {
                Ok(f) => f,
                Err(e) => {
                    if e.kind() == io::ErrorKind::NotFound {
                        tokio::fs::create_dir(IMAGE_DIR).await.unwrap();
                        File::create(&file_path).await.expect("should be created")
                    } else {
                        panic!("{}", e)
                    }
                }
            };
            while let Some(chunk) = field.chunk().await.unwrap() {
                file.write_all(&chunk).await.unwrap();
            }
        }
    }

    let new_room = match ChatManager::new(&state.pool)
        .new_room(&builder.name.unwrap(), &builder.image_path.unwrap(), &user)
        .await
    {
        Ok(room) => Some(room),
        Err(_) => None,
    };

    NewRoomResultsTemplate { room: new_room }
}

#[derive(Template)]
#[template(path = "new_room.html")]
pub struct NewRoomTemplate {}

pub async fn new_room() -> NewRoomTemplate {
    NewRoomTemplate {}
}
