#![warn(rust_2018_idioms)]

const SHAPE_SIZE: usize = 13;
const SHAPE: [&str; 5] = ["spade", "diamond", "heart", "clover", "none"];
const JOKER: &str = "joker";
const NUM: [&str; 13] = [
    "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A",
];
const FRIEND_INDICATE: [&str; 4] = ["none", "first turn", "last turn", "by card"];
const CARD_FRIEND_NUM: usize = 3;
const KIRUDA_COEF: usize = 1000; //누가먹나판별시사용
const FIRST_SHAPE_COEF: usize = 100; //누가먹나판별시사용
const MIGHTY_COEF: usize = 100000; //누가먹나판별시사용
const JOKER_COEF: usize = 10000; //누가먹나판별시사용
const MIGHTY_SHAPE: usize = 0;
const MIGHTY_NUM: usize = 12;
const JOKERCALL_SHAPE: usize = 3;
const JOKERCALL_NUM: usize = 1;
const JOKER_SHAPE: usize = 4;
const EMPTY_SHAPE: usize = 5; //필드위에 아직 안냈음 표현할때 사용
const LEAST_EAT_NUM: usize = 14;
const LEAST_SCORE_NUM: usize = 8;
const NO_KIRUDA_SHAPE: usize = 4;
const BACKRUN_NUM: usize = 10;

use rand::Rng;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::task::yield_now;
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

use futures::SinkExt;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    // Configure a `tracing` subscriber that logs traces emitted by the chat
    // server.
    tracing_subscriber::fmt()
        // Filter what traces are displayed based on the RUST_LOG environment
        // variable.
        //
        // Traces emitted by the example code will always be displayed. You
        // can set `RUST_LOG=tokio=trace` to enable additional traces emitted by
        // Tokio itself.
        .with_env_filter(EnvFilter::from_default_env().add_directive("chat=info".parse()?))
        // Log events when `tracing` spans are created, entered, exited, or
        // closed. When Tokio's internal tracing support is enabled (as
        // described above), this can be used to track the lifecycle of spawned
        // tasks on the Tokio runtime.
        .with_span_events(FmtSpan::FULL)
        // Set this subscriber as the default, to collect all traces emitted by
        // the program.
        .init();

    // Create the shared state. This is how all the peers communicate.
    //
    // The server task will hold a handle to this. For every new client, the
    // `state` handle is cloned and passed into the task that processes the
    // client connection.
    let state = Arc::new(Mutex::new(Shared::new()));

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:6142".to_string());

    // Bind a TCP listener to the socket address.
    //
    // Note that this is the Tokio TcpListener, which is fully async.
    let listener = TcpListener::bind(&addr).await?;

    tracing::info!("server running on {}", addr);

    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;

        // Clone a handle to the `Shared` state for the new connection.
        let state = Arc::clone(&state);

        // Spawn our handler to be run asynchronously.
        tokio::spawn(async move {
            tracing::debug!("accepted connection");
            if let Err(e) = process(state, stream, addr).await {
                tracing::info!("an error occurred; error = {:?}", e);
            }
        });
    }
}

struct Person {
    id: i32,
}
impl Person {
    fn copy(&self) -> Person {
        Person { id: self.id }
    }
}
struct Card {
    shape: usize, //0이 스페이드, 1이 다이아, 2가 하트, 3이 클로버,4 조커
    num: usize,   //A가 12,2가 0
}
impl Card {
    fn copy(&self) -> Card {
        Card {
            shape: self.shape,
            num: self.num,
        }
    }
}
struct Player {
    person: Person,
    card_in_hand: Vec<Card>,
    card_eaten: Vec<Card>,
}
struct Field {
    cards: Vec<Card>,
    first_shape: usize,
}
struct Friend {
    how_indicate: usize,
    card: Card,
}

#[derive(PartialEq, Eq, Hash)]
struct Quickinfo {
    roomid: String,
    order: i32,
}

struct Shared {
    lazydata: HashMap<Quickinfo, Framed<TcpStream, LinesCodec>>,
}


/// The state for each connected client.

impl Shared {
    /// Create a new, empty, instance of `Shared`.
    fn new() -> Self {
        Shared {
            lazydata: HashMap::new(),
        }
    }

    async fn movedata(&mut self, lines: Framed<TcpStream, LinesCodec>, roomid: String, order: i32) {
        self.lazydata.insert(
            Quickinfo {
                roomid: format!("{}", roomid),
                order,
            },
            lines,
        );
    }

    async fn getnum(&mut self, roomid: String) -> i32 {
        let mut num = 0;
        for peer in self.lazydata.iter_mut() {
            if peer.0.roomid == roomid {
                num = num + 1;
            }
        }
        return num;
    }

    async fn message(&mut self, message: &str, roomid: String) {
        for i in [1, 2, 3, 4, 5] {
            let key = Quickinfo {
                roomid: format!("{}", roomid),
                order: i,
            };

            if let Some(peer) = self.lazydata.get_mut(&key) {
                peer.send(message).await;
            }
        }
    }

    async fn dm(&mut self, message: &str, roomid: String, order: i32) {
        let key = Quickinfo {
            roomid: format!("{}", roomid),
            order,
        };

        if let Some(peer) = self.lazydata.get_mut(&key) {
            peer.send(message).await;
        }
    }

    async fn playmighty(&mut self, roomid: String) {
        self.message("Game Start!", format!("{}", roomid)).await;

        for i in [1, 2, 3, 4, 5] {
            let msg = format!("You go {}th", i);

            let key = Quickinfo {
                roomid: format!("{}", roomid),
                order: i,
            };

            if let Some(peer) = self.lazydata.get_mut(&key) {
                peer.send(&msg).await;
            }
        }

        let p1 = Person { id: 1 };
        let p2 = Person { id: 2 };
        let p3 = Person { id: 3 };
        let p4 = Person { id: 4 };
        let p5 = Person { id: 5 };
        let person_list = vec![p1, p2, p3, p4, p5];
        let pan_number = 8;
        let mut dealer: usize = 0;
        let mut score: [usize; 5] = [0, 0, 0, 0, 0];

        for i in 0..pan_number {
            self.message(&format!("{}th game", i + 1), format!("{}", roomid))
                .await;
            self.play_game(&person_list, &mut dealer, &mut score, format!("{}", roomid))
                .await;
            self.printscore(&person_list, &mut score, format!("{}", roomid))
                .await;
        }

        let mut winner: usize = 0;

        for i in 0..5 {
            if score[winner] < score[i] {
                winner = i;
            }
        }
        self.message(
            &format!("Final winner : {}", person_list[winner].id),
            format!("{}", roomid),
        )
        .await;
    }

    async fn printscore(
        &mut self,
        person_list: &Vec<Person>,
        score: &mut [usize; 5],
        roomid: String,
    ) {
        for i in 0..5 {
            self.message(
                &format!("Player {}'s score is {}", i + 1, score[i]),
                format!("{}", roomid),
            )
            .await;
        }
    }

    fn divid_cards() -> (
        Vec<Card>,
        Vec<Card>,
        Vec<Card>,
        Vec<Card>,
        Vec<Card>,
        Vec<Card>,
    ) {
        //randomize
        let mut random: Vec<usize> = Vec::new();
        while random.len() < 53 {
            let mut x: usize = rand::thread_rng().gen_range(0..53);
            let mut flag: bool = true;
            for j in 0..random.len() {
                if x == random[j] {
                    flag = false;
                }
            }
            if flag {
                random.push(x);
            }
        }

        let mut a1: Vec<Card> = Vec::new();
        for i in 0..10 {
            a1.push(Card {
                shape: (random[i] / SHAPE_SIZE),
                num: (random[i] % SHAPE_SIZE),
            })
        }
        let mut a2: Vec<Card> = Vec::new();
        for i in 10..20 {
            a2.push(Card {
                shape: (random[i] / SHAPE_SIZE),
                num: (random[i] % SHAPE_SIZE),
            })
        }
        let mut a3: Vec<Card> = Vec::new();
        for i in 20..30 {
            a3.push(Card {
                shape: (random[i] / SHAPE_SIZE),
                num: (random[i] % SHAPE_SIZE),
            })
        }
        let mut a4: Vec<Card> = Vec::new();
        for i in 30..40 {
            a4.push(Card {
                shape: (random[i] / SHAPE_SIZE),
                num: (random[i] % SHAPE_SIZE),
            })
        }
        let mut a5: Vec<Card> = Vec::new();
        for i in 40..50 {
            a5.push(Card {
                shape: (random[i] / SHAPE_SIZE),
                num: (random[i] % SHAPE_SIZE),
            })
        }
        let mut a6: Vec<Card> = Vec::new();
        for i in 50..53 {
            a6.push(Card {
                shape: (random[i] / SHAPE_SIZE),
                num: (random[i] % SHAPE_SIZE),
            })
        }
        (a1, a2, a3, a4, a5, a6)
    }

    async fn play_game(
        &mut self,
        person_list: &Vec<Person>,
        dealer: &mut usize,
        score: &mut [usize; 5],
        roomid: String,
    ) {
        let (A1, A2, A3, A4, A5, under_deck) = Shared::divid_cards(); // Dividing cards by rand
        let (n1, n2, n3, n4, n5) = (0, 1, 2, 3, 4); //set_number_assign
        let mut player_list: Vec<Player> = vec![
            Player {
                person: person_list[n1].copy(),
                card_in_hand: A1,
                card_eaten: Vec::new(),
            },
            Player {
                person: person_list[n2].copy(),
                card_in_hand: A2,
                card_eaten: Vec::new(),
            },
            Player {
                person: person_list[n3].copy(),
                card_in_hand: A3,
                card_eaten: Vec::new(),
            },
            Player {
                person: person_list[n4].copy(),
                card_in_hand: A4,
                card_eaten: Vec::new(),
            },
            Player {
                person: person_list[n5].copy(),
                card_in_hand: A5,
                card_eaten: Vec::new(),
            },
        ];
        //show_card(&player_list); 분배 멀쩡히 되는지 시험용
        let mut startpoint: usize = 0;
        let (jugongnum, musteatnum, kiruda, friend) = self
            .pick_jugong(
                &mut player_list,
                &under_deck,
                &startpoint,
                format!("{}", roomid),
            )
            .await;
        startpoint = jugongnum;
        let mut friendnum = jugongnum;
        for i in 0..10 {
            self.message(&format!("{}th round", i + 1), format!("{}", roomid))
                .await;
            self.playround(
                &mut player_list,
                &i,
                &mut startpoint,
                &kiruda,
                &friend,
                &mut friendnum,
                format!("{}", roomid),
            )
            .await;
            self.show_eaten(&player_list, &jugongnum, &friendnum, format!("{}", roomid))
                .await;
        }
        //모든 라운드 종료

        let score1 = Shared::score_calc(&player_list[jugongnum], &player_list[friendnum]);
        *dealer = friendnum;
        self.message("all rounds are over", format!("{}", roomid))
            .await;
        self.message(
            &format!("점수 : {} / 공약 : {}", score1, musteatnum),
            format!("{}", roomid),
        )
        .await;
        let mut backrun_coef = 1;
        if score1 <= BACKRUN_NUM {
            backrun_coef = 2;
            *dealer = jugongnum;
        }

        if score1 >= musteatnum {
            self.message("president wins!", format!("{}", roomid)).await;
            let mut no_kiru_coef = 1;
            if kiruda == NO_KIRUDA_SHAPE {
                no_kiru_coef = 2;
            }
            let mut jumsu =
                ((score1 - LEAST_EAT_NUM) + (musteatnum - LEAST_EAT_NUM) * 2) * no_kiru_coef;
            score[jugongnum] = score[jugongnum] + 2 * jumsu;
            if jugongnum != friendnum {
                score[friendnum] = score[friendnum] + jumsu;
            } else {
                score[jugongnum] = score[jugongnum] + 2 * jumsu;
            }
            for i in 0..5 {
                if i != friendnum && i != jugongnum {
                    score[i] = score[i] - jumsu;
                }
            }
        } else {
            self.message("president loses!", format!("{}", roomid))
                .await;
            let jumsu = (musteatnum - score1) * backrun_coef;
            score[jugongnum] = score[jugongnum] - 2 * jumsu;
            if jugongnum != friendnum {
                score[friendnum] = score[friendnum] - jumsu;
            } else {
                score[jugongnum] = score[jugongnum] - 2 * jumsu;
            }
            for i in 0..5 {
                if i != friendnum && i != jugongnum {
                    score[i] = score[i] + jumsu;
                }
            }
        }
    }

    async fn pick_jugong(
        &mut self,
        player_list: &mut Vec<Player>,
        under_deck: &Vec<Card>,
        startpoint: &usize,
        roomid: String,
    ) -> (usize, usize, usize, Friend) {
        let mut jugongnum: usize = 0;
        let mut musteatnum: usize = LEAST_EAT_NUM - 1;
        let mut kiruda: usize = 0;
        let mut position = *startpoint;
        let mut did_passed: Vec<usize> = vec![0, 0, 0, 0, 0];
        let mut passed_num = 0;
        let mut anyone_chulma: usize = 0;
        let mut friend = Friend {
            how_indicate: 0,
            card: Card {
                shape: EMPTY_SHAPE,
                num: 0,
            },
        };

        while position < 6 {
            //주공정하기
            if did_passed[position] == 0 {
                self.dm(
                    &format!("{}'s turn", player_list[position].person.id),
                    format!("{}", roomid),
                    player_list[position].person.id,
                )
                .await;
                self.dm(
                    &format!("{}'s card", player_list[position].person.id),
                    format!("{}", roomid),
                    player_list[position].person.id,
                )
                .await;

                self.printcard(
                    &player_list[position].card_in_hand,
                    format!("{}", roomid),
                    player_list[position].person.id,
                )
                .await;

                self.dm(
                    "pass is 0, candidancy is 1",
                    format!("{}", roomid),
                    player_list[position].person.id,
                )
                .await;

                let mut go_or_not = String::new();

                let key = Quickinfo {
                    roomid: format!("{}", roomid),
                    order: player_list[position].person.id,
                };

                if let Some(peer) = self.lazydata.get_mut(&key) {
                    let result = peer.next().await;
                    match result {
                        Some(Ok(msg)) => {
                            go_or_not = msg;
                        }
                        Some(Err(e)) => {
                            go_or_not = format!("2");
                        }
                        None => {}
                    }
                }

                let go_or_not: usize = go_or_not.trim().parse().expect("Please type valid value!");
                if go_or_not == 1 {
                    loop {
                        self.dm(
                            &format!(
                                "Suit : 0 is {}, 1 is {}, 2 is {}, 3 is {}, {} is nokiruda",
                                SHAPE[0], SHAPE[1], SHAPE[2], SHAPE[3], NO_KIRUDA_SHAPE
                            ),
                            format!("{}", roomid),
                            player_list[position].person.id,
                        )
                        .await;
                        let mut shape = String::new();

                        let key = Quickinfo {
                            roomid: format!("{}", roomid),
                            order: player_list[position].person.id,
                        };

                        if let Some(peer) = self.lazydata.get_mut(&key) {
                            let result = peer.next().await;
                            match result {
                                Some(Ok(msg)) => {
                                    shape = msg;
                                }
                                Some(Err(e)) => {
                                    shape = format!("4");
                                }
                                None => {}
                            }
                        }

                        let shape: usize = shape.trim().parse().expect("Please type valid value!");
                        if shape > 4 || shape < 0 {
                            self.dm(
                                "Please type valid value!",
                                format!("{}", roomid),
                                player_list[position].person.id,
                            ).await;
                        } else {
                            let mut is_kiruda_exist = 1;
                            if shape == NO_KIRUDA_SHAPE {
                                is_kiruda_exist = 0;
                            }
                            self.dm(
                                &format!(
                                    "Make your campaign: the minimum is {}.",
                                    musteatnum + is_kiruda_exist
                                ),
                                format!("{}", roomid),
                                player_list[position].person.id,
                            )
                            .await;

                            let mut num = String::new();

                            let key = Quickinfo {
                                roomid: format!("{}", roomid),
                                order: player_list[position].person.id,
                            };

                            if let Some(peer) = self.lazydata.get_mut(&key) {
                                let result = peer.next().await;
                                match result {
                                    Some(Ok(msg)) => {
                                        num = msg;
                                    }
                                    Some(Err(e)) => {
                                        num = format!("21");
                                    }
                                    None => {}
                                }
                            }
                            let num: usize = num.trim().parse().expect("Please type valid value!");
                            if num >= musteatnum + is_kiruda_exist && num <= 20 {
                                jugongnum = position;
                                kiruda = shape;
                                musteatnum = num;
                                break;
                            }
                        }
                    } //end of loop
                    anyone_chulma = 1;
                    self.message(
                        &format!(
                            "{} became candidate with campaign {}",
                            player_list[position].person.id, musteatnum
                        ),
                        format!("{}", roomid),
                    )
                    .await;
                } else if go_or_not == 0 {
                    did_passed[position] = 1;
                    passed_num = passed_num + 1;
                }
            }
            if musteatnum == 20 {
                break;
            }
            if passed_num >= 4 && anyone_chulma == 1 {
                break;
            }
            if passed_num == 5 && anyone_chulma == 0 {
                for i in 0..5 {
                    did_passed[i] = 0;
                }
                passed_num = 0;
                musteatnum = musteatnum - 1;
            }
            position = (position + 1) % 5;
        } //주공정하기 끝, end of while
        self.message(
            &format!(
                "President {} : Campaign {} , {}",
                player_list[jugongnum].person.id, SHAPE[kiruda], musteatnum
            ),
            format!("{}", roomid),
        )
        .await;
        //밑패보기
        for i in 0..3 {
            player_list[jugongnum]
                .card_in_hand
                .push(under_deck[i].copy());
        }
        for i in 0..3 {
            let mut num: usize = 0;
            loop {
                self.printcard(
                    &player_list[jugongnum].card_in_hand,
                    format!("{}", roomid),
                    player_list[jugongnum].person.id,
                )
                .await;
                self.dm(
                    "Choose discarding card (by number) : ",
                    format!("{}", roomid),
                    player_list[jugongnum].person.id,
                )
                .await;
                let mut num1 = String::new();
                let key = Quickinfo {
                    roomid: format!("{}", roomid),
                    order: player_list[jugongnum].person.id,
                };

                if let Some(peer) = self.lazydata.get_mut(&key) {
                    let result = peer.next().await;
                    match result {
                        Some(Ok(msg)) => {
                            num1 = msg;
                        }
                        Some(Err(e)) => {
                            num1 = format!("14");
                        }
                        None => {}
                    }
                }
                num = num1.trim().parse().expect("Please type valid value!");
                //num 범위체크
                if num <= player_list[jugongnum].card_in_hand.len() && num > 0 {
                    break;
                } else {
                    self.dm(
                        "Type correct value",
                        format!("{}", roomid),
                        player_list[jugongnum].person.id,
                    )
                    .await;
                }
            }
            let c = player_list[jugongnum].card_in_hand[num - 1].copy();
            player_list[jugongnum].card_eaten.push(c);
            player_list[jugongnum].card_in_hand.remove(num - 1);
        }
        self.printcard(
            &player_list[jugongnum].card_in_hand,
            format!("{}", roomid),
            player_list[jugongnum].person.id,
        )
        .await;
        self.dm(
            "Do you wish to change kiruda? Yes 1 No 0 :",
            format!("{}", roomid),
            player_list[jugongnum].person.id,
        )
        .await;
        let mut num = String::new();
        let key = Quickinfo {
            roomid: format!("{}", roomid),
            order: player_list[jugongnum].person.id,
        };

        if let Some(peer) = self.lazydata.get_mut(&key) {
            let result = peer.next().await;
            match result {
                Some(Ok(msg)) => {
                    num = msg;
                }
                Some(Err(e)) => {
                    num = format!("14");
                }
                None => {}
            }
        }

        let num: usize = num.trim().parse().expect("Please type valid value!");
        if num == 1 {
            loop {
                self.dm(
                    &format!(
                        "Suit to change? : 0 is {}, 1 is {}, 2 is {}, 3 is {}, {}is no kiruda",
                        SHAPE[0], SHAPE[1], SHAPE[2], SHAPE[3], NO_KIRUDA_SHAPE
                    ),
                    format!("{}", roomid),
                    player_list[jugongnum].person.id,
                )
                .await;
                let mut shape = String::new();
                let key = Quickinfo {
                    roomid: format!("{}", roomid),
                    order: player_list[jugongnum].person.id,
                };
                if let Some(peer) = self.lazydata.get_mut(&key) {
                    let result = peer.next().await;
                    match result {
                        Some(Ok(msg)) => {
                            shape = msg;
                        }
                        Some(Err(e)) => {
                            shape = format!("14");
                        }
                        None => {}
                    }
                }
                let shape: usize = shape.trim().parse().expect("Please type valid value!");

                if shape > 4 || shape < 0 {
                    self.dm(
                        "Type correct value",
                        format!("{}", roomid),
                        player_list[jugongnum].person.id,
                    )
                    .await;
                } else {
                    let mut is_kiruda_exist = 1;
                    if shape == NO_KIRUDA_SHAPE {
                        is_kiruda_exist = 0;
                    }
                    self.dm(
                        &format!(
                            "Make your campaign: the minimum is {}.",
                            musteatnum + 1 + is_kiruda_exist
                        ),
                        format!("{}", roomid),
                        player_list[jugongnum].person.id,
                    )
                    .await;
                    let mut num = String::new();

                    let key = Quickinfo {
                        roomid: format!("{}", roomid),
                        order: player_list[jugongnum].person.id,
                    };

                    if let Some(peer) = self.lazydata.get_mut(&key) {
                        let result = peer.next().await;
                        match result {
                            Some(Ok(msg)) => {
                                num = msg;
                            }
                            Some(Err(e)) => {
                                num = format!("14");
                            }
                            None => {}
                        }
                    }

                    let num: usize = num.trim().parse().expect("Please type valid value!");
                    if num >= musteatnum + 1 + is_kiruda_exist && num <= 20 {
                        kiruda = shape;
                        musteatnum = num;
                        break;
                    }
                }
            } //end of loop
        }
        //친구정하기

        let mut how_ind: usize = 0;
        loop {
            //지정방식고르기
            self.printcard(
                &player_list[jugongnum].card_in_hand,
                format!("{}", roomid),
                player_list[jugongnum].person.id,
            )
            .await;
            self.dm(
                "Choose a way of selecting partner",
                format!("{}", roomid),
                player_list[jugongnum].person.id,
            )
            .await;
            for i in 0..FRIEND_INDICATE.len() {
                self.dm(
                    &format!("{} : {}  ", i, FRIEND_INDICATE[i]),
                    format!("{}", roomid),
                    player_list[jugongnum].person.id,
                )
                .await;
            }
            let mut num1 = String::new();
            let key = Quickinfo {
                roomid: format!("{}", roomid),
                order: player_list[jugongnum].person.id,
            };

            if let Some(peer) = self.lazydata.get_mut(&key) {
                let result = peer.next().await;
                match result {
                    Some(Ok(msg)) => {
                        num1 = msg;
                    }
                    Some(Err(e)) => {
                        num1 = format!("99");
                    }
                    None => {}
                }
            }
            how_ind = num1.trim().parse().expect("Please type valid value!");
            //num 범위체크
            if how_ind < FRIEND_INDICATE.len() && how_ind >= 0 {
                break;
            } else {
                self.dm(
                    "Please type valid value!",
                    format!("{}", roomid),
                    player_list[jugongnum].person.id,
                )
                .await;
            }
        }
        friend.how_indicate = how_ind;
        //카드지정일경우 지정할 카드 입력받기
        if how_ind == CARD_FRIEND_NUM {
            loop {
                self.dm(
                    &format!(
                        "Suit : 0 is {}, 1 is {}, 2 is {}, 3 is {}, {} is joker",
                        SHAPE[0], SHAPE[1], SHAPE[2], SHAPE[3], JOKER_SHAPE
                    ),
                    format!("{}", roomid),
                    player_list[jugongnum].person.id,
                )
                .await;

                let mut shape = String::new();
                let key = Quickinfo {
                    roomid: format!("{}", roomid),
                    order: player_list[jugongnum].person.id,
                };
                if let Some(peer) = self.lazydata.get_mut(&key) {
                    let result = peer.next().await;
                    match result {
                        Some(Ok(msg)) => {
                            shape = msg;
                        }
                        Some(Err(e)) => {
                            shape = format!("5");
                        }
                        None => {}
                    }
                }
                let shape: usize = shape.trim().parse().expect("Please type valid value!");
                if shape > 4 || shape < 0 {
                    self.dm(
                        "Please type valid value!",
                        format!("{}", roomid),
                        player_list[jugongnum].person.id,
                    )
                    .await;
                } else {
                    if shape == JOKER_SHAPE {
                        friend.card.shape = shape;
                        break;
                    }
                    self.dm(
                        "Choose a number!",
                        format!("{}", roomid),
                        player_list[jugongnum].person.id,
                    )
                    .await;
                    for i in 0..NUM.len() {
                        self.dm(
                            &format!("{} : {}  ", i, NUM[i]),
                            format!("{}", roomid),
                            player_list[jugongnum].person.id,
                        )
                        .await;
                    }
                    let mut num = String::new();
                    let key = Quickinfo {
                        roomid: format!("{}", roomid),
                        order: player_list[jugongnum].person.id,
                    };
                    if let Some(peer) = self.lazydata.get_mut(&key) {
                        let result = peer.next().await;
                        match result {
                            Some(Ok(msg)) => {
                                num = msg;
                            }
                            Some(Err(e)) => {
                                num = format!("20");
                            }
                            None => {}
                        }
                    };
                    let num: usize = num.trim().parse().expect("Please type valid value!");
                    if num >= 0 && num < SHAPE_SIZE {
                        friend.card.shape = shape;
                        friend.card.num = num;
                        break;
                    }
                }
            } //end of loop
        }
        // 주공/공약정보 출력하고 리턴
        self.message(
            &format!(
                "President {} : Campaign {} , {}",
                player_list[jugongnum].person.id, SHAPE[kiruda], musteatnum
            ),
            format!("{}", roomid),
        )
        .await;
        (jugongnum, musteatnum, kiruda, friend)
    }

    async fn printcard(&mut self, cards: &Vec<Card>, roomid: String, order: i32) {
        let mut n = 1;
        for i in cards {
            if i.shape == JOKER_SHAPE {
                self.dm(
                    &format!("{}th card : {}", n, JOKER),
                    format!("{}", roomid),
                    order,
                )
                .await;
            } else {
                self.dm(
                    &format!("{}th card : {} {}", n, SHAPE[i.shape], NUM[i.num]),
                    format!("{}", roomid),
                    order,
                )
                .await;
            }
            n = n + 1;
        }
    }

    async fn playround(
        &mut self,
        player_list: &mut Vec<Player>,
        roundnum: &usize,
        startpoint: &mut usize,
        kiruda: &usize,
        friend: &Friend,
        friendnum: &mut usize,
        roomid: String,
    ) {
        let mut field = Field {
            cards: vec![
                Card { shape: 5, num: 0 },
                Card { shape: 5, num: 0 },
                Card { shape: 5, num: 0 },
                Card { shape: 5, num: 0 },
                Card { shape: 5, num: 0 },
            ],
            first_shape: 5,
        };
        let mut position = *startpoint;
        let mut is_joker_called: usize = 0;
        for i in 0..5 {
            self.dm(
                &format!("{}'s turn", player_list[position].person.id),
                format!("{}", roomid),
                player_list[position].person.id,
            ).await;

            self.printfield(&field, &player_list, format!("{}", roomid))
                .await;

            self.dm(
                &format!("{}'s card", player_list[position].person.id),
                format!("{}", roomid),
                player_list[position].person.id,
            )
            .await;

            self.printcard(
                &player_list[position].card_in_hand,
                format!("{}", roomid),
                player_list[position].person.id,
            )
            .await;
            let mut num: usize = 0;
            loop {
                //카드 입력받기
                loop {
                    self.dm(
                        "Type the card to play",
                        format!("{}", roomid),
                        player_list[position].person.id,
                    )
                    .await;
                    let mut num1 = String::new();

                    let key = Quickinfo {
                        roomid: format!("{}", roomid),
                        order: player_list[position].person.id,
                    };
                    if let Some(peer) = self.lazydata.get_mut(&key) {
                        let result = peer.next().await;
                        match result {
                            Some(Ok(msg)) => {
                                num1 = msg;
                            }
                            Some(Err(e)) => {
                                num1 = format!("20");
                            }
                            None => {}
                        }
                    };
                    num = num1.trim().parse().expect("Please type valid value!");
                    num = num - 1;
                    if num >= 0 && num < player_list[position].card_in_hand.len() {
                        break;
                    } else {
                        self.dm(
                            "Type proper value",
                            format!("{}", roomid),
                            player_list[position].person.id,
                        )
                        .await;
                    }
                }
                //낼 수 있는 카드인지 체크하기
                //조커콜당한경우
                if is_joker_called == 1 {
                    //조커있으면 조커내야함
                    let mut have_joker = 0;
                    let mut joker_num = 0;
                    let mut have_mighty = 0;
                    let mut mighty_num = 0;
                    for i in 0..player_list[position].card_in_hand.len() {
                        if player_list[position].card_in_hand[i].shape == JOKER_SHAPE {
                            have_joker = 1;
                            joker_num = i;
                        }
                    }
                    for i in 0..player_list[position].card_in_hand.len() {
                        if player_list[position].card_in_hand[i].shape
                            == MIGHTY_SHAPE + Shared::is_mighty_affected(&kiruda)
                            && player_list[position].card_in_hand[i].num == MIGHTY_NUM
                        {
                            have_mighty = 1;
                            mighty_num = i;
                        }
                    }
                    if have_joker == 1 && have_mighty == 0 {
                        if player_list[position].card_in_hand[num].shape == JOKER_SHAPE {
                            break;
                        } else {
                            self.dm(
                                "please play joker",
                                format!("{}", roomid),
                                player_list[position].person.id,
                            )
                            .await;
                            continue;
                        }
                    } else if have_joker == 1 && have_mighty == 1 {
                        if player_list[position].card_in_hand[num].shape == JOKER_SHAPE {
                            break;
                        } else if player_list[position].card_in_hand[num].shape
                            == MIGHTY_SHAPE + Shared::is_mighty_affected(&kiruda)
                            && player_list[position].card_in_hand[num].num == MIGHTY_NUM
                        {
                            break;
                        } else {
                            self.dm(
                                "please play joker or mighty",
                                format!("{}", roomid),
                                player_list[position].person.id,
                            )
                            .await;
                            continue;
                        }
                    }
                }
                //선인 경우 처리 - 선과 같은문양 통과 - 마이티통과 - 조커통과 - 선과 다른문양이면 검정 순서
                if field.first_shape == 5 {
                    if *roundnum == 0 && player_list[position].card_in_hand[num].shape == *kiruda {
                        let mut n = 0;
                        for i in 0..player_list[position].card_in_hand.len() {
                            if player_list[position].card_in_hand[i].shape != *kiruda {
                                n = 1;
                            }
                        }
                        if n == 0 {
                            break;
                        }
                    } else if player_list[position].card_in_hand[num].shape == JOKER_SHAPE {
                        //초구 문양 정하기
                        let mut shape: usize = 0;
                        loop {
                            self.dm(
                                "choose your suit : 0 is spade, 1 is diamond, 2 is clover, 3 is clover",
                                format!("{}", roomid),
                                player_list[position].person.id,
                            )
                            .await;
                            let mut shape1 = String::new();

                            let key = Quickinfo {
                                roomid: format!("{}", roomid),
                                order: player_list[position].person.id,
                            };
                            if let Some(peer) = self.lazydata.get_mut(&key) {
                                let result = peer.next().await;
                                match result {
                                    Some(Ok(msg)) => {
                                        shape1 = msg;
                                    }
                                    Some(Err(e)) => {
                                        shape1 = format!("20");
                                    }
                                    None => {}
                                }
                            };

                            shape = shape1.trim().parse().expect("Please type valid value!");
                            if shape > 3 || shape < 0 {
                                self.dm(
                                    "Type proper value",
                                    format!("{}", roomid),
                                    player_list[position].person.id,
                                )
                                .await;
                            } else {
                                break;
                            }
                        }
                        field.first_shape = shape;
                        break;
                    } else if *roundnum > 0
                        && player_list[position].card_in_hand[num].shape
                            == (JOKERCALL_SHAPE + Shared::is_jokercall_affected(kiruda)) % 4
                        && player_list[position].card_in_hand[num].num == JOKERCALL_NUM
                    {
                        field.first_shape = player_list[position].card_in_hand[num].shape;
                        is_joker_called = 1;
                        self.dm(
                            "Type proper value",
                            format!("{}", roomid),
                            player_list[position].person.id,
                        )
                        .await;
                        break;
                    } else {
                        field.first_shape = player_list[position].card_in_hand[num].shape;
                        break;
                    }
                    //초구처리완료
                } else if field.first_shape == player_list[position].card_in_hand[num].shape {
                    break; //선과같은거내면 통과
                } else if player_list[position].card_in_hand[num].shape
                    == MIGHTY_SHAPE + Shared::is_mighty_affected(&kiruda)
                    && player_list[position].card_in_hand[num].num == MIGHTY_NUM
                {
                    break; //마이티내도통과
                } else if player_list[position].card_in_hand[num].shape == JOKER_SHAPE {
                    break; //조커내도통과
                } else {
                    let mut have_first_shape: usize = 0;
                    for i in 0..player_list[position].card_in_hand.len() {
                        if player_list[position].card_in_hand[i].shape == field.first_shape {
                            self.dm(
                                "Play same suit with the starting player",
                                format!("{}", roomid),
                                player_list[position].person.id,
                            )
                            .await;
                            have_first_shape = 1;
                            break;
                        }
                    }
                    if have_first_shape == 0 {
                        break; //초구와 같은거 없으면 통과
                    }
                } //end_of_if_else
            } //end_of_loop
              //프렌카드이면 프렌만들기
            if friend.how_indicate == CARD_FRIEND_NUM
                && friend.card.shape == player_list[position].card_in_hand[num].shape
                && friend.card.num == player_list[position].card_in_hand[num].num
            {
                *friendnum = position;
                self.message(
                    &format!("friend : {}", player_list[position].person.id),
                    format!("{}", roomid),
                )
                .await;
            }
            //필드에 카드 넣기, 그 카드 패에서 지우기
            field.cards[position] = player_list[position].card_in_hand[num].copy();
            player_list[position].card_in_hand.remove(num);
            position = (position + 1) % 5;
        } //카드 다 돌림
          //누가 먹는지 결정
        *startpoint = self
            .eat_field(
                &field,
                player_list,
                kiruda,
                roundnum,
                &is_joker_called,
                format!("{}", roomid),
            )
            .await;
        //초구프렌,막구프렌
        if friend.how_indicate == 1 && *roundnum == 0 {
            *friendnum = *startpoint;
            self.message(
                &format!("friend : {}", player_list[*friendnum].person.id),
                format!("{}", roomid),
            )
            .await;
        }
        if friend.how_indicate == 2 && *roundnum == 9 {
            *friendnum = *startpoint;
            self.message(
                &format!("friend : {}", player_list[*friendnum].person.id),
                format!("{}", roomid),
            )
            .await;
        }
    }

    async fn eat_field(
        &mut self,
        field: &Field,
        player_list: &mut Vec<Player>,
        kiruda: &usize,
        roundnum: &usize,
        is_joker_called: &usize,
        roomid: String,
    ) -> usize {
        self.printfield(&field, &player_list, format!("{}", roomid)).await;
        let mut strong: [usize; 5] = [0, 0, 0, 0, 0];
        let mut strongest: usize = 0;
        for n in 0..5 {
            strong[n] = field.cards[n].num;
            if field.cards[n].shape == field.first_shape {
                strong[n] = strong[n] + FIRST_SHAPE_COEF;
            }
            if field.cards[n].shape == *kiruda && *kiruda != NO_KIRUDA_SHAPE {
                strong[n] = strong[n] + KIRUDA_COEF;
            }
            if field.cards[n].shape == MIGHTY_SHAPE + Shared::is_mighty_affected(&kiruda)
                && field.cards[n].num == MIGHTY_NUM
            {
                strong[n] = strong[n] + MIGHTY_COEF;
            }
            if field.cards[n].shape == JOKER_SHAPE {
                strong[n] = strong[n]
                    + JOKER_COEF * Shared::is_joker_avaliable(roundnum) * (1 - *is_joker_called);
            }
        }
        for n in 0..5 {
            if strong[n] > strong[strongest] {
                strongest = n;
            }
        }
        self.message(
            &format!("{} wins!", player_list[strongest].person.id),
            format!("{}", roomid),
        )
        .await;
        for n in 0..5 {
            player_list[strongest]
                .card_eaten
                .push(field.cards[n].copy());
        }
        strongest
    }

    async fn show_eaten(
        &mut self,
        player_list: &Vec<Player>,
        jugong: &usize,
        friend: &usize,
        roomid: String,
    ) {
        self.message("List of cards taken", format!("{}", roomid))
            .await;
        for i in 0..5 {
            self.message(
                &format!("Card taken by {}", player_list[i].person.id),
                format!("{}", roomid),
            )
            .await;
            if i != *jugong && i != *friend {
                let mut n = 1;
                for j in 0..player_list[i].card_eaten.len() {
                    if player_list[i].card_eaten[j].shape != JOKER_SHAPE
                        && player_list[i].card_eaten[j].num >= LEAST_SCORE_NUM
                    {
                        self.message(
                            &format!(
                                "{} {}",
                                SHAPE[player_list[i].card_eaten[j].shape],
                                NUM[player_list[i].card_eaten[j].num]
                            ),
                            format!("{}", roomid),
                        )
                        .await;
                        n = n + 1;
                    }
                }
            } else {
            }
        }
    }

    async fn printfield(&mut self, field: &Field, player_list: &Vec<Player>, roomid: String) {
        self.message("Cards on the field", format!("{}", roomid))
            .await;
        for n in 0..5 {
            if field.cards[n].shape == EMPTY_SHAPE {
                self.message(
                    &format!("{}'s card : none", player_list[n].person.id),
                    format!("{}", roomid),
                )
                .await;
            } else if field.cards[n].shape < 4 && field.cards[n].shape >= 0 {
                self.message(
                    &format!(
                        "{}'s card : {} {}",
                        player_list[n].person.id,
                        SHAPE[field.cards[n].shape],
                        NUM[field.cards[n].num]
                    ),
                    format!("{}", roomid),
                )
                .await;
            } else if field.cards[n].shape == JOKER_SHAPE {
                self.message(
                    &format!("{}의 card : {}", player_list[n].person.id, JOKER),
                    format!("{}", roomid),
                )
                .await;
            }
        }
        if field.first_shape != EMPTY_SHAPE {
            self.message(
                &format!("First card {}", SHAPE[field.first_shape]),
                format!("{}", roomid),
            )
            .await;
        }
    }

    fn is_joker_avaliable(roundnum: &usize) -> usize {
        let mut n = 1;
        if *roundnum == 0 || *roundnum == 9 {
            n = 0;
        }
        n
    }
    fn is_mighty_affected(kiruda: &usize) -> usize {
        let mut n: usize = 0;
        if *kiruda == MIGHTY_SHAPE {
            n = 1;
        }
        n
    }
    fn is_jokercall_affected(kiruda: &usize) -> usize {
        let mut n: usize = 0;
        if *kiruda == JOKERCALL_SHAPE {
            n = 1;
        }
        n
    }
    fn score_calc(jugong: &Player, friend: &Player) -> usize {
        let mut score: usize = 0;
        for i in &jugong.card_eaten {
            if i.num > 8 {
                score = score + 1;
            }
        }
        for i in &friend.card_eaten {
            if i.num > 8 {
                score = score + 1;
            }
        }
        score
    }
}

/// Process an individual chat client
async fn process(
    state: Arc<Mutex<Shared>>,
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    let mut lines = Framed::new(stream, LinesCodec::new());

    // Send a prompt to the client to enter their username.
    lines.send("Please enter your username:").await?;

    // Read the first line from the `LineCodec` stream to get the username.
    let username = match lines.next().await {
        Some(Ok(line)) => line,
        // We didn't get a line so we return early here.
        _ => {
            tracing::error!("Failed to get username from {}. Client disconnected.", addr);
            return Ok(());
        }
    };

    lines.send("Please enter your room ID:").await?;

    let roomid = match lines.next().await {
        Some(Ok(line)) => line,
        // We didn't get a line so we return early here.
        _ => {
            tracing::error!("Failed to get room ID from {}. Client disconnected.", addr);
            return Ok(());
        }
    };

    //After getting data from the user, send the data of user to the server

    {
        let mut state = state.lock().await;
        let roomid = format!("{}", roomid);
        let roomid1 = format!("{}", roomid);
        let ord = state.getnum(roomid).await + 1;
        state.movedata(lines, roomid1, ord).await;
    }

    {
        let mut state = state.lock().await;
        let msg = format!("{} has joined room {}", username, roomid);
        let roomid = format!("{}", roomid);
        state.message(&msg, roomid).await;
    }

    // 5th member triggers the start of the game
    {
        let mut state = state.lock().await;
        let roomid = format!("{}", roomid);
        let roomid1 = format!("{}", roomid);

        if state.getnum(roomid).await == 5 {
            state.playmighty(roomid1).await;
        }
    }

    loop {
        yield_now().await;
    }

    Ok(())
}
