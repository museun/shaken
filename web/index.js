import Vue from "vue";

var lines = [];

var app = new Vue({
    el: '#chat',
    data: function () { return { lines: lines } },
    destroyed: disconnect,
});

const MAX = 64;

const socket = new ReconnectingWebSocket("ws://localhost:51001");
socket.addEventListener('open', function (event) {
    socket.send(JSON.stringify("42")); // this should be a real handshake
});

socket.addEventListener('message', function (event) {
    let data = JSON.parse(event.data);
    if (lines.length >= MAX) {
        lines.shift();
    }

    let pos = lines.findIndex((el) => {
        return el.timestamp < data.timestamp;
    });

    if (pos == -1 || pos == 0) {
        lines.push(data);
    } else {
        lines.splice(pos, 0, data);
    }

    console.log(`<${data.display}> ${data.data}`);
});

const disconnect = () => { socket.close(); }
