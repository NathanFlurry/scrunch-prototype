use ws::{Handler, Sender as WsSender, Result as WsResult, Message, CloseCode, Handshake, Error};
use std::collections::HashSet;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::io::Cursor;
use std::fmt::{Display, Formatter, Result as FormatResult};
use rmpv::Value;
use rmpv::encode::write_value;
use rmpv::decode::read_value;
use game::{GameReference};
use game_map::{GameMap, MapIndex, IndexType};
use entities::{EntityId};

/// Errors that occur when processing messages
#[derive(Debug)]
enum MessageError {
    Malformed,
    MissingData,
    EventType
}

impl Display for MessageError {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        write!(f, "{:?}", self)
    }
}

/// Type used to identify clients
pub type ClientId = u64;

/// Used by the game to
pub struct ClientHandle {
    out: WsSender,

    watching_entities: HashSet<EntityId>,

    spectating_index: MapIndex, /// Index at which the player is spectating from; this is used for if the player dies and doesn't have a position
    pub player_id: Option<EntityId>,

    pub rx_join: Receiver<String>,
    pub rx_move: Receiver<MapIndex>,
    pub rx_leave: Receiver<()>
}

impl SocketSender for ClientHandle {
    fn socket_out(&self) -> &WsSender {
        &self.out
    }
}

impl ClientHandle {
    /// Distance from the player that can be seen
    const VIEW_RANGE: IndexType = 15;

    pub fn validate_player_ids(&mut self, map: &GameMap) {
        if let Some(id) = self.player_id {
            if !map.entities().contains_key(&id) {
                self.player_id = None;
            }
        }
    }

    fn index_in_range(&self, base: &MapIndex, index: &MapIndex) -> bool {
            index.x <= base.x + ClientHandle::VIEW_RANGE &&
            index.x >= base.x - ClientHandle::VIEW_RANGE &&
            index.y <= base.y + ClientHandle::VIEW_RANGE &&
            index.y >= base.y - ClientHandle::VIEW_RANGE
    }

    pub fn build_update_message(&mut self, map: &GameMap) {
        /* Determine items */
        // Create lists to hold the data
        let mut appeared_entities = Vec::<EntityId>::new();
        let mut updated_entities = Vec::<EntityId>::new();
        let mut disappeared_entities = Vec::<EntityId>::new(); // Entities that moved out of range
        let mut destroyed_entities = Vec::<EntityId>::new();

        // Get the basic index
        if let Some(ref player_id) = self.player_id {
            if let Some(player_object) = map.entity_with_id(player_id) {
                self.spectating_index.clone_from(player_object.index());
            }
        }

        // Add moved entities
        for id in self.watching_entities.iter() {
            if let Some(entity) = map.entity_with_id(id) {
                // Update entity if in range; otherwise remove it
                if self.index_in_range(&self.spectating_index, entity.index()) {
                    if entity.needs_update() {
                        updated_entities.push(id.clone());
                    }
                } else {
                    disappeared_entities.push(id.clone());
                }
            } else if map.destroyed_entities().contains_key(id) {
                destroyed_entities.push(id.clone());
            } else {
                // The entity isn't in the map and it wasn't destroyed, so where'd it go?
                println!("Unable to find entity with ID {}", id);
            }
        }

        // Find new entities to start watching
        for (id, object) in map.entities().iter() {
            if !self.watching_entities.contains(id) && self.index_in_range(&self.spectating_index, object.index()) {
                appeared_entities.push(id.clone());
            }
        }

        /* Serialize message */
        let message = Value::Array(vec![
            map.map_size().into(),
            appeared_entities.iter()
                .map(|id| (id, map.entity_with_id(id))) // Find the entity on the map
                .filter(|&(_, entity)| entity.is_some()) // Make sure it exists
                .map(|(id, entity)|  entity.unwrap().serialize(id, true)) // Serialize it
                .collect::<Vec<Value>>().into(),
            updated_entities.iter()
                .map(|id| (id, map.entity_with_id(id))) // Find the entity on the map
                .filter(|&(_, entity)| entity.is_some()) // Make sure it exists
                .map(|(id, entity)|  entity.unwrap().serialize(id, false)) // Serialize it
                .collect::<Vec<Value>>().into(),
            disappeared_entities.iter()
                .map(|id| Value::from(id.clone()))
                .collect::<Vec<Value>>().into(),
            destroyed_entities.iter()
                .map(|id| Value::from(id.clone()))
                .collect::<Vec<Value>>().into()
        ]);
        self.send_update(message);

        /* Update watching entities list */
        // Add and remove entities
        for id in appeared_entities.into_iter() {
            self.watching_entities.insert(id);
        }
        for id in disappeared_entities.iter() {
            self.watching_entities.remove(id);
        }
        for id in destroyed_entities.iter() {
            self.watching_entities.remove(&id);
        }
    }
}

/// Handles incoming connections; one is created for every individual WebSocket connection created.
/// This just forwards important messages to the connection itself.
pub struct Client {
    out: WsSender,
    is_open: bool,

    tx_join: Sender<String>,
    tx_move: Sender<MapIndex>,
    tx_leave: Sender<()>
}

impl Client {
    pub fn new(game: GameReference, out: WsSender) -> Client {
        // Create the channels
        let (tx_join, rx_join) = channel();
        let (tx_move, rx_move) = channel();
        let (tx_leave, rx_leave) = channel();

        // Register the connection with the game
        {
            let mut game = game.lock().unwrap();
            game.add_client(ClientHandle {
                out: out.clone(),
                watching_entities: HashSet::new(),
                spectating_index: MapIndex::new(0,0),
                player_id: None,
                rx_join, rx_move, rx_leave
            });
        }

        // Return the server
        Client {
            out, is_open: false,
            tx_join, tx_move, tx_leave
        }
    }

    fn close(&mut self) {
        // Send leave message
        self.tx_leave.send(()).unwrap();

        // Close the socket
        self.out.close(CloseCode::Empty).unwrap();

        // Set to not open
        self.is_open = false;
    }
}

impl Handler for Client {
    fn on_open(&mut self, shake: Handshake) -> WsResult<()> {
        println!("Connection: {:?}", shake.request.client_addr());

        // Set to open
        self.is_open = true;

        Ok(())
    }

    fn on_message(&mut self, msg: Message) -> WsResult<()> {
        match self.parse_message(msg) {
            Ok(_) => Ok(()),
            Err(err) => {
                // TODO: Send error back, maybe with Err(...)
                println!("Error: {}", err);
                Ok(())
            }
        }
    }

    fn on_close(&mut self, code: CloseCode, reason: &str) {
        // Print the error
        match code {
            CloseCode::Normal => println!("The client is done with the connection."),
            CloseCode::Away => println!("The client is leaving the site."),
            _ => println!("The client encountered an error: {}", reason),
        }

        self.close();
    }

    fn on_error(&mut self, err: Error) {
        println!("Socket error: {}", err);

        self.close();
    }
}

impl SocketSender for Client {
    fn socket_out(&self) -> &WsSender {
        &self.out
    }
}

/* Message handlers */
impl Client {
    fn parse_message(&self, msg: Message) -> Result<(), MessageError> {
        // Get the data from the message
        let message_value = match msg {
            Message::Binary(data) => {
                // Create a cursor to read the data and convert it to a MessagePack value
                let mut cursor = Cursor::new(data);
                read_value(&mut cursor).map_err(|_| MessageError::Malformed)?
            }
            Message::Text(_) => return Err(MessageError::Malformed)
        };

        // Parse the base of the message
        let message: &Vec<Value> = unwrap_data!(message_value.as_array());
        if message.len() != 2 {
            return Err(MessageError::MissingData)
        }
        let message_type = unwrap_data!(message[0].as_u64());
        let message_body = &message[1];

        // Handle the message
        match message_type {
            0 => self.join_game(message_body),
            1 => self.move_player(message_body),
            _ => Err(MessageError::EventType)
        }
    }

    fn join_game(&self, data: &Value) -> Result<(), MessageError> { // TODO: Use `data`
        // Get the username
        let username: String = unwrap_data!(data.as_str()).to_string();

        // Send the message
        self.tx_join.send(username).unwrap();

        Ok(())
    }

    fn move_player(&self, data: &Value) -> Result<(), MessageError> {
        // Parse the data
        let index_raw: &Vec<Value> = unwrap_data!(data.as_array());
        if index_raw.len() != 2 {
            return Err(MessageError::MissingData)
        }

        // Destruct the data to a map index and send the message
        let index = if let (Some(x), Some(y)) = (index_raw[0].as_i64(), index_raw[1].as_i64()) {
            MapIndex::new(x, y)
        } else {
            return Err(MessageError::MissingData)
        };

        // Send the message
        self.tx_move.send(index).unwrap();

        Ok(())
    }
}

/* Socket sender */
/// Declares the type of message
pub enum MessageType {
    Join,
    Update
}

impl MessageType {
    pub fn message_flag(&self) -> u8 {
        match self {
            &MessageType::Join => 0,
            &MessageType::Update => 1
        }
    }
}

/// Trait used to easily serialize and send messages.
pub trait SocketSender {
    fn socket_out(&self) -> &WsSender;

    fn send_message(&self, message_type: MessageType, message_body: Value) {
        // Create new message
        let message = Value::Array(vec![Value::from(message_type.message_flag()), message_body]);

        // Serialize the message
        let mut buf = Vec::new();
        write_value(&mut buf, &message).unwrap();

        // Send the message
        self.socket_out().send(buf).unwrap();
    }

    fn send_join(&self, player_id: EntityId) {
        self.send_message(MessageType::Join, Value::from(player_id));
    }

    fn send_update(&self, message: Value) {
        // Generate the data
        self.send_message(MessageType::Update, message);
    }
}
