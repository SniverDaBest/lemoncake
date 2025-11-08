const log = console.log;
const error_face = "(X_X)"
const error_msg = "[ERROR]: Unable to find this page!"

const warning_face = "(0_0)"
const warning_msg = "[WARN ]: Couldn't find this page!"

const info_face = "(o_o)"
const info_msg = "[INFO ]: Didn't find this page."

const todo_face = "(-_-)"
const todo_msg = "[TODO ]: Find this page."

let footer = document.createElement("footer");
let footertext = document.createElement("p");
footertext.id = "footertext";
footertext.innerHTML = `Created by SniverDaBest :D
<br>
<a href="license">License</a>
<a href="https://github.com/SniverDaBest/lemoncake">GitHub</a>
<a href="https://github.com/SniverDaBest/lemoncake/tree/gh-pages">GitHub (this site)</a>`;
footer.appendChild(footertext);
document.body.appendChild(footer);

function gohome() {
    document.location.href = "/lemoncake"
}

function copied(tag) {
    if (tag.startsWith("link")) {
        let link = document.getElementById(tag);
        link.innerText = " Copied!"
        navigator.clipboard.writeText(`${document.location.href}#${tag}`).then(function() {
            log(`Copied "${document.location.href}#${tag}"`);
        });
        setTimeout(function() {
            link.innerText = "";
        }, 2000);
    } else {
        let link = document.getElementById(`link${tag}`);
        link.innerText = " Copied!"
        navigator.clipboard.writeText(`${document.location.href}#${tag}`).then(function() {
            log(`Copied "${document.location.href}#${tag}"`);
        });
        setTimeout(function() {
            link.innerText = "";
        }, 2000);
    }
}

function changeMode() {
    alert("(-_-) [TODO ]: Implement switching from dark to light mode! (and vice versa)");
}

let changeErrorCount = 0;

function changeError() {
    let face = document.getElementById("bigerror");
    let message = document.getElementById("bigerror2");
    
    changeErrorCount += 1;

    if (changeErrorCount >= 34) {
        face.className = "error";
        message.className = "error";
        face.innerText = "(T_T)";
        message.innerText = "[DUMB ]: OH COME ON! I'm done with this stupid game. I'm booting you back to the homepage.";
        setTimeout(gohome, 3000);
        return;
    } else if (changeErrorCount >= 33) {
        alert("Yeah, I left. I wasn't kidding.");
        face.className = "invis";
        message.className = "invis";
        face.innerText = "(o_o)";
        message.innerText = "[INVIS]: You'll never find me! hehehe"
        return;
    } else if (changeErrorCount >= 32) {
        face.className = "todo";
        message.className = "todo";
        face.innerText = "(-_-)";
        message.innerHTML = "[STOP ]: If you press this <b id='red'>ONE MORE TIME</b>, I will leave."
        return;
    } else if (changeErrorCount >= 30) {
        face.className = "error";
        message.className = "error";
        face.innerText = "(>_<)";
        message.innerHTML = "[PLEAS]: Please, just <b>STOP</b> pressing this button! I won't tell you again."
        return;
    } else if (changeErrorCount >= 25) {
        face.className = "srsly";
        message.className = "srsly";
        face.innerText = "(T_T)";
        message.innerHTML = "[SRSLY]: Can you like, stop pressing this button? It can't possibly be <i>that</i> fun..."
        return;
    }

    if (face.className == "error") {
        face.className = "warning";
        message.className = "warning";
        face.innerText = warning_face;
        message.innerText = warning_msg;
    } else if (face.className == "warning") {
        face.className = "info";
        message.className = "info";
        face.innerText = info_face;
        message.innerText = info_msg;
    } else if (face.className == "info") {
        face.className = "todo";
        message.className = "todo";
        face.innerText = todo_face;
        message.innerText = todo_msg;
    } else if (face.className == "todo") {
        face.className = "error";
        message.className = "error";
        face.innerText = error_face;
        message.innerText = error_msg;
    }
}