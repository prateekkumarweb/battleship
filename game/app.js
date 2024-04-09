document.addEventListener("alpine:init", () => {
  Alpine.data("chat", () => ({
    messages: [],
    newMessage: "",
    ws: null,
    id: null,
    opponentId: null,
    userBoard: Array.from({ length: 10 }, () =>
      Array.from({ length: 10 }, () => ({
        hit: false,
        miss: false,
        ship: false,
      }))
    ),
    opponentBoard: Array.from({ length: 10 }, () =>
      Array.from({ length: 10 }, () => ({
        hit: false,
        miss: false,
        ship: false,
      }))
    ),
    userShips: [null, null, null, null, null],
    opponentShips: [null, null, null, null, null],
    showPlaceShips: true,
    myTurn: false,
    youWin: false,
    oppWin: false,
    init() {
      this.generateRandomShips();
      this.ws = new WebSocket(this.constructWsUrl("/ws"));
      this.ws.addEventListener("open", (event) => {
        this.ws.send(JSON.stringify({ type: "Connect" }));
      });
      this.ws.addEventListener("message", (event) => {
        const data = JSON.parse(event.data);
        console.log("Received", data);
        switch (data.type) {
          case "Id":
            this.id = data.data;
            break;
          case "OpponentId":
            this.addMessage("log", "System", "Opponent found " + data.data);
            this.opponentId = data.data;
            break;
          case "Log":
            this.addMessage("log", "System", data.data);
            break;
          case "Chat":
            this.addMessage("chat", "Opponent", data.data);
            break;
          case "StartGame":
            this.showPlaceShips = false;
            this.addMessage("log", "System", "Game started");
            break;
          case "NotYourTurn":
            this.addMessage("log", "System", "Not your turn");
            break;
          case "Hit":
            this.opponentBoard[data.data[1]][data.data[0]].hit = true;
            break;
          case "Miss":
            this.opponentBoard[data.data[1]][data.data[0]].miss = true;
            break;
          case "OppHit":
            this.userBoard[data.data[1]][data.data[0]].hit = true;
            break;
          case "OppMiss":
            this.userBoard[data.data[1]][data.data[0]].miss = true;
            break;
          case "YourTurn":
            this.addMessage("log", "System", "Your turn");
            this.myTurn = true;
            break;
          case "OppTurn":
            this.addMessage("log", "System", "Opponent's turn");
            this.myTurn = false;
            break;
          case "Sank":
            let ship = data.data.ship;
            this.opponentShips[data.data.index] = ship;
            this.placeShip(
              ship.x,
              ship.y,
              ship.size,
              ship.vertical,
              this.opponentBoard
            );
            break;
          case "YouWin":
            this.addMessage("log", "System", "You win");
            this.youWin = true;
            break;
          case "OppWin":
            this.addMessage("log", "System", "Opponent wins");
            this.oppWin = true;
            break;
          case "Close":
            this.addMessage("log", "System", "Opponent disconnected");
            this.ws.close();
            break;
          default:
            console.error("Unknown message type", data);
        }
      });
      this.ws.addEventListener("error", (event) => {
        console.error(event);
        this.addMessage("error", "System", "An error occurred");
      });
      this.ws.addEventListener("close", (event) => {
        this.addMessage("error", "System", "Connection closed");
      });
    },
    constructWsUrl(newPath) {
      let loc = window.location;
      let newUrl;

      if (loc.protocol === "https:") {
        newUrl = "wss:";
      } else {
        newUrl = "ws:";
      }

      newUrl += "//" + loc.host;
      newUrl += newPath;

      return newUrl;
    },
    newBoard() {
      return Array.from({ length: 10 }, () =>
        Array.from({ length: 10 }, () => ({
          hit: false,
          miss: false,
          ship: false,
        }))
      );
    },
    sendMessage() {
      if (this.newMessage !== "") {
        let message = JSON.stringify({ type: "Chat", data: this.newMessage });
        this.ws.send(message);
        this.addMessage("chat", "Me", this.newMessage);
        this.newMessage = "";
      }
    },
    addMessage(type, from, data) {
      this.messages.push({ type, from, data });
      this.$nextTick(() => {
        const chatWindow = document.querySelector(".overflow-y-scroll");
        chatWindow.scrollTop = chatWindow.scrollHeight;
      });
    },
    generateRandomShips() {
      let ships = [5, 4, 3, 3, 2];
      this.userBoard = this.newBoard();
      for (let [index, size] of ships.entries()) {
        let placed = false;
        while (!placed) {
          let x = Math.floor(Math.random() * 10);
          let y = Math.floor(Math.random() * 10);
          let vertical = Math.random() > 0.5;
          if (this.canPlaceShip(x, y, size, vertical, this.userBoard)) {
            this.placeShip(x, y, size, vertical, this.userBoard);
            this.userShips[index] = {
              x,
              y,
              size,
              vertical,
              sunk: false,
            };
            placed = true;
          }
        }
      }
      console.log(this.userBoard);
    },
    canPlaceShip(x, y, ship, vertical, board) {
      if (vertical) {
        if (y + ship > 10) {
          return false;
        }
        for (let i = 0; i < ship; i++) {
          if (board[y + i][x].ship) {
            return false;
          }
        }
      } else {
        if (x + ship > 10) {
          return false;
        }
        for (let i = 0; i < ship; i++) {
          if (board[y][x + i].ship) {
            return false;
          }
        }
      }
      return true;
    },
    placeShip(x, y, ship, vertical, board) {
      if (vertical) {
        for (let i = 0; i < ship; i++) {
          board[y + i][x].ship = true;
          board[y + i][x].vertical = vertical;
        }
        board[y][x].shipStart = true;
        board[y + ship - 1][x].shipEnd = true;
      } else {
        for (let i = 0; i < ship; i++) {
          board[y][x + i].ship = true;
          board[y][x + i].vertical = vertical;
        }
        board[y][x].shipStart = true;
        board[y][x + ship - 1].shipEnd = true;
      }
    },
    hasShip(x, y, board) {
      return board[y][x].ship;
    },
    showMarginTop(x, y, board) {
      if (board[y][x].vertical) {
        if (board[y][x].shipStart) {
          return true;
        } else {
          return false;
        }
      } else {
        return true;
      }
    },
    showMarginBottom(x, y, board) {
      if (board[y][x].vertical) {
        if (board[y][x].shipEnd) {
          return true;
        } else {
          return false;
        }
      } else {
        return true;
      }
    },
    showMarginLeft(x, y, board) {
      if (!board[y][x].vertical) {
        if (board[y][x].shipStart) {
          return true;
        } else {
          return false;
        }
      } else {
        return true;
      }
    },
    showMarginRight(x, y, board) {
      if (!board[y][x].vertical) {
        if (board[y][x].shipEnd) {
          return true;
        } else {
          return false;
        }
      } else {
        return true;
      }
    },
    startGame() {
      this.ws.send(JSON.stringify({ type: "StartGame", data: this.userShips }));
    },
    move(x, y) {
      console.log("Move", x, y);
      this.ws.send(JSON.stringify({ type: "Move", data: [x, y] }));
    },
  }));
});
