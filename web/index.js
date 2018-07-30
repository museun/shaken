import Vue from "vue";

var lines = [];

var app = new Vue({
    el: '#chat',
    data: function () { return { lines: lines } },
    destroyed: disconnect,
});

const MAX = 64;

// is this const a problem?

const socket = new WebSocket("ws://localhost:51000");
socket.addEventListener('open', function (event) {
    socket.send(JSON.stringify("42")); // this should be a real handshake
});
socket.addEventListener('message', function (event) {
    let data = JSON.parse(event.data);
    if (lines.length >= MAX) {
        lines.shift();
    }

    lines.push(data);
    // they won't have the same timestamp
    lines.sort((a, b) => { return a.timestamp < b.timestamp ? -1 : 1; });

    console.log(`<${data.display}> ${data.data}`);
});

const disconnect = () => { socket.close(); }
