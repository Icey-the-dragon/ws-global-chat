const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
const host = window.location.host;

const loginOverlay = document.getElementById('login-overlay');
const loginButton = document.getElementById('login-button');
const registerButton = document.getElementById('register-button');
const usernameInput = document.getElementById('username');
const passwordInput = document.getElementById('password');
const loginError = document.getElementById('login-error');
const loginSuccess = document.getElementById('login-success');
const chatBox = document.getElementById('chat-box'); // Kept from original, not explicitly removed by edit
const messagesDiv = document.getElementById('messages');
const input = document.getElementById('input');
const logoutButton = document.getElementById('logout-button');

let socket;
let currentSessionToken = null;

async function checkSession() {
    try {
        const response = await fetch('/api/me');
        if (response.ok) {
            const data = await response.json();
            if (data.valid) {
                currentSessionToken = data.session_token;
                startChat();
            } else {
                loginOverlay.classList.remove('hidden');
            }
        } else {
            console.log("Session invalid or expired (Status: " + response.status + ")");
            loginOverlay.classList.remove('hidden');
        }
    } catch (err) {
        console.error("Error during session check:", err);
        loginOverlay.classList.remove('hidden');
    }
}

async function handleAuth(endpoint) {
    const username = usernameInput.value;
    const password = passwordInput.value;

    if (!username || !password) {
        showError("Please enter both username and password");
        return;
    }

    try {
        const response = await fetch(`/api/${endpoint}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password })
        });

        const data = await response.json();

        if (response.ok) {
            showSuccess(data.message);
            currentSessionToken = data.session_token;
            startChat();
        } else {
            showError(data || "Authentication failed");
        }
    } catch (err) {
        showError("Server error, please try again later");
    }
}

loginButton.onclick = () => handleAuth('login');
registerButton.onclick = () => handleAuth('register');

logoutButton.onclick = async () => {
    try {
        await fetch('/api/logout', { method: 'POST' });
        currentSessionToken = null;
        location.reload();
    } catch (err) {
        console.error("Logout failed", err);
    }
};

function showError(msg) {
    loginError.textContent = msg;
    loginError.classList.remove('hidden');
    loginSuccess.classList.add('hidden');
}

function showSuccess(msg) {
    loginSuccess.textContent = msg;
    loginSuccess.classList.remove('hidden');
    loginError.classList.add('hidden');
}

async function load_history(limit) {
    try {
        const response = await fetch('/api/get_chat_history?limit=' + limit, {
            method: 'GET',
        });

        const data = await response.json();

        if (response.ok) {
            data.forEach(current_message => {
                const messageDiv = document.createElement('div');
                messageDiv.className = 'message';
                messageDiv.innerHTML = `<strong>${current_message.username}:</strong> ${current_message.content}`;
                messages.appendChild(messageDiv);
                messages.scrollTop = messages.scrollHeight;
            });
        } else {
            showError(data || "Unable to retrieve the chat_history");
        }
    } catch (err) {
        showError("Could not connect with server");
    }
}

function startChat() {
    loginOverlay.classList.add('hidden');
    chatBox.classList.remove('hidden');
    input.focus();

    socket = new WebSocket(`${protocol}//${host}/ws`);

    load_history(50);

    socket.onmessage = function (event) {
        const msg = JSON.parse(event.data);
        const messageElement = document.createElement('div');
        messageElement.className = 'message';

        switch (msg.type) {
            case 'private':
                messageElement.classList.add('private-message');
                messageElement.innerHTML = `<strong>[PM] ${msg.username}:</strong> ${msg.content}`;
                break;
            case 'ephemeral':
                messageElement.classList.add('ephemeral-message');
                messageElement.innerHTML = `<strong>[EPHEMERAL] ${msg.username}:</strong> ${msg.content}`;
                // Emit custom event with extra metadata for client-to-client comms
                if (msg.extra) {
                    window.dispatchEvent(new CustomEvent('ephemeral', { detail: msg }));
                }
                break;
            case 'who':
                // Internal server probe — display as system info
                messageElement.classList.add('system-message');
                messageElement.innerHTML = `<strong>[SYSTEM]</strong> ${msg.content}`;
                break;
            case 'error':
                messageElement.classList.add('error-message');
                messageElement.innerHTML = `<strong>[ERROR]</strong> ${msg.content}`;
                break;
            case 'broadcast':
            default:
                messageElement.innerHTML = `<strong>${msg.username}:</strong> ${msg.content}`;
                break;
        }

        messagesDiv.appendChild(messageElement);
        messagesDiv.scrollTop = messagesDiv.scrollHeight;
    };

    socket.onclose = function () {
        console.log("WebSocket connection closed");
    };
}

//send message function
input.addEventListener('keypress', (event) => {
    if (event.key === 'Enter' && input.value.trim() !== '' && socket) {
        const text = input.value.trim();
        let msg;

        if (text.startsWith('/pm ')) {
            // Format: /pm @username message
            const parts = text.substring(4).match(/^@(\S+)\s+(.*)/);
            if (parts) {
                msg = {
                    type: "private",
                    metadata: { session_id: currentSessionToken, to_username: parts[1] },
                    content: parts[2]
                };
            }
        } else if (text.startsWith('/ephemeral ')) {
            msg = {
                type: "ephemeral",
                metadata: { session_id: currentSessionToken },
                content: text.substring(11)
            };
        } else {
            msg = {
                type: "broadcast",
                metadata: { session_id: currentSessionToken },
                content: text
            };
        }

        if (msg) {
            socket.send(JSON.stringify(msg));
        }
        input.value = '';
    }
});
logoutButton.addEventListener('click', async () => {
    try {
        await fetch('/api/logout', { method: 'POST' });
        window.location.reload();
    } catch (err) {
        console.error("Logout failed", err);
        window.location.reload();
    }
});

checkSession();
