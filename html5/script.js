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
const currentUsernameSpan = document.getElementById('current-username');

let socket;
let currentSessionToken = null;
let currentUsername = null;
let suggestionMenu = null;
let onlineUsers = []; // Keep for backward compatibility, but won't be populated
let usernameQueryTimeout = null;
let selectedSuggestionIndex = -1;
let currentSuggestions = [];
let currentOnSelect = null;
let lastSelectedUsername = null;

// Hide login overlay initially to prevent flash if user has valid session
loginOverlay.classList.add('hidden');

function createSuggestionMenu() {
    if (suggestionMenu) return;
    suggestionMenu = document.createElement('div');
    suggestionMenu.id = 'suggestion-menu';
    suggestionMenu.style.position = 'absolute';
    suggestionMenu.style.background = 'rgba(255, 255, 255, 0.05)';
    suggestionMenu.style.border = '1px solid rgba(255, 255, 255, 0.1)';
    suggestionMenu.style.borderRadius = '12px';
    suggestionMenu.style.boxShadow = '0 8px 32px 0 rgba(0, 0, 0, 0.37)';
    suggestionMenu.style.maxHeight = '200px';
    suggestionMenu.style.overflowY = 'auto';
    suggestionMenu.style.zIndex = '1000';
    suggestionMenu.style.backdropFilter = 'blur(10px)';
    document.body.appendChild(suggestionMenu);
}

function showSuggestions(suggestions, onSelect) {
    if (!suggestionMenu) createSuggestionMenu();
    suggestionMenu.innerHTML = '';
    currentSuggestions = suggestions;
    currentOnSelect = onSelect;
    selectedSuggestionIndex = -1;

    suggestions.forEach((suggestion, index) => {
        const item = document.createElement('div');
        item.textContent = suggestion;
        item.style.padding = '10px 15px';
        item.style.cursor = 'pointer';
        item.style.color = '#fff';
        item.style.transition = 'background 0.2s ease';
        item.addEventListener('click', () => {
            onSelect(suggestion);
            hideSuggestions();
        });
        item.addEventListener('mouseenter', () => {
            updateSelection(index);
        });
        suggestionMenu.appendChild(item);
    });

    positionMenu();
    suggestionMenu.style.display = 'block';
}

function updateSelection(newIndex) {
    // Remove previous selection
    if (selectedSuggestionIndex >= 0 && selectedSuggestionIndex < suggestionMenu.children.length) {
        suggestionMenu.children[selectedSuggestionIndex].style.background = 'transparent';
    }

    selectedSuggestionIndex = newIndex;

    // Apply new selection
    if (selectedSuggestionIndex >= 0 && selectedSuggestionIndex < suggestionMenu.children.length) {
        suggestionMenu.children[selectedSuggestionIndex].style.background = 'rgba(233, 69, 96, 0.3)';
        // Scroll into view
        suggestionMenu.children[selectedSuggestionIndex].scrollIntoView({
            block: 'nearest',
            inline: 'nearest'
        });
    }
}

function hideSuggestions() {
    if (suggestionMenu) {
        suggestionMenu.style.display = 'none';
    }
    selectedSuggestionIndex = -1;
    currentSuggestions = [];
    currentOnSelect = null;
}

function positionMenu() {
    const rect = input.getBoundingClientRect();
    const menuHeight = Math.min(200, suggestionMenu.scrollHeight);
    suggestionMenu.style.left = rect.left + 'px';
    suggestionMenu.style.bottom = (window.innerHeight - rect.top + 2) + 'px';
    suggestionMenu.style.width = rect.width + 'px';
}

async function queryUsernames(prefix) {
    try {
        const response = await fetch('/api/usernames_by_prefix', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ prefix })
        });
        if (response.ok) {
            return await response.json();
        }
    } catch (err) {
        console.error('Failed to query usernames:', err);
    }
    return [];
}

async function checkSession() {
    try {
        const response = await fetch('/api/me');
        if (response.ok) {
            const data = await response.json();
            if (data.valid) {
                currentSessionToken = data.session_token;
                currentUsername = data.username;
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
            currentUsername = data.username;
            startChat();
        } else {
            if (response.status === 409) {
                showError("Username already taken");
            } else {
                showError(data || "Authentication failed");
            }
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
    currentUsernameSpan.textContent = currentUsername;
    input.focus();

    socket = new WebSocket(`${protocol}//${host}/ws`);

    load_history(50);

    socket.onmessage = function (event) {
        if (event.data.startsWith("403 ")) {
            window.location.reload();
        }
        const msg = JSON.parse(event.data);
        const messageElement = document.createElement('div');
        messageElement.className = 'message';

        switch (msg.type) {
            case 'private':
                messageElement.classList.add('private-message');
                const isSender = msg.to_username && msg.to_username !== currentUsername;
                const label = isSender
                    ? `[PM to ${msg.to_username}]`
                    : `[PM from ${msg.username}]`;
                messageElement.innerHTML = `<strong>${label}</strong> ${msg.content}`;
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
                if (msg.to_username && msg.to_username === currentUsername && socket) {
                    socket.send(JSON.stringify({
                        type: "who",
                        metadata: { session_id: currentSessionToken },
                        content: ""
                    }));
                }
                return;
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
input.addEventListener('input', (event) => {
    const text = input.textContent;
    hideSuggestions();
    if (text.includes('@')) {
        // Username completion after @
        const atIndex = text.indexOf('@');
        const prefix = text.substring(atIndex + 1);
        if (prefix.length >= 1) { // Require at least 1 character before querying
            // Debounce the query
            clearTimeout(usernameQueryTimeout);
            usernameQueryTimeout = setTimeout(async () => {
                const matches = await queryUsernames(prefix.toLowerCase());
                if (matches.length > 0) {
                    showSuggestions(matches, (selected) => {
                        const beforeAt = text.substring(0, atIndex);
                        lastSelectedUsername = selected;
                        input.innerHTML = beforeAt + '<span class="mention">@' + selected.trim() + '</span>&nbsp;';
                        input.focus();
                        // Move cursor to end
                        const range = document.createRange();
                        const sel = window.getSelection();
                        range.selectNodeContents(input);
                        range.collapse(false);
                        sel.removeAllRanges();
                        sel.addRange(range);
                        hideSuggestions();
                    });
                }
            }, 300); // Increased debounce to 300ms
        }
    }
});

input.addEventListener('keydown', (event) => {
    if (suggestionMenu && suggestionMenu.style.display !== 'none') {
        // Handle suggestion navigation
        if (event.key === 'ArrowDown') {
            event.preventDefault();
            const newIndex = Math.min(selectedSuggestionIndex + 1, currentSuggestions.length - 1);
            updateSelection(newIndex);
        } else if (event.key === 'ArrowUp') {
            event.preventDefault();
            const newIndex = Math.max(selectedSuggestionIndex - 1, -1);
            updateSelection(newIndex);
        } else if (event.key === 'Enter' && selectedSuggestionIndex >= 0) {
            event.preventDefault();
            if (currentOnSelect && currentSuggestions[selectedSuggestionIndex]) {
                currentOnSelect(currentSuggestions[selectedSuggestionIndex]);
                hideSuggestions();
            }
        } else if (event.key === 'Escape') {
            hideSuggestions();
        }
    } else {
        // Command and username auto-completion with Ctrl+Space
        if (event.ctrlKey && event.code === 'Space') {
            event.preventDefault();
            const text = input.textContent;

            // Check for username completion (@x pattern)
            if (text.includes('@')) {
                const atIndex = text.lastIndexOf('@');
                const prefix = text.substring(atIndex + 1);

                // Check if @ is followed by at least 1 character and no spaces
                if (prefix.length >= 1 && !prefix.includes(' ')) {
                    // Query usernames
                    queryUsernames(prefix.toLowerCase()).then(matches => {
                        if (matches.length > 0) {
                            showSuggestions(matches, (selected) => {
                                const beforeAt = text.substring(0, atIndex + 1);
                                lastSelectedUsername = selected;
                                input.innerHTML = beforeAt + '<span class="mention">@' + selected.trim() + '</span>&nbsp;';
                                input.focus();
                                // Move cursor to end
                                const range = document.createRange();
                                const sel = window.getSelection();
                                range.selectNodeContents(input);
                                range.collapse(false);
                                sel.removeAllRanges();
                                sel.addRange(range);
                                hideSuggestions();
                            });
                        }
                    });
                    return;
                }
            }

            // Check if input starts with /
            if (text.startsWith('/')) {
                const commands = [
                    '/pm @username message',
                    '/ephemeral message'
                ];

                // Filter commands that start with the input
                const matches = commands.filter(cmd => cmd.startsWith(text));

                if (matches.length === 1) {
                    // Auto-complete if only one match
                    const selected = matches[0];
                    if (selected.startsWith('/pm')) {
                        input.textContent = '/pm @';
                        input.focus();
                        // Set cursor after @
                        const range = document.createRange();
                        const sel = window.getSelection();
                        range.setStart(input.firstChild, 5);
                        range.setEnd(input.firstChild, 5);
                        sel.removeAllRanges();
                        sel.addRange(range);
                    } else if (selected.startsWith('/ephemeral')) {
                        input.innerHTML = '/ephemeral&nbsp;';
                        input.focus();
                        // Set cursor at end
                        const range = document.createRange();
                        const sel = window.getSelection();
                        range.selectNodeContents(input);
                        range.collapse(false);
                        sel.removeAllRanges();
                        sel.addRange(range);
                    }
                    input.focus();
                } else if (matches.length > 1) {
                    // Show suggestions if multiple matches
                    showSuggestions(matches, (selected) => {
                        if (selected.startsWith('/pm')) {
                            input.textContent = '/pm @';
                            input.focus();
                            // Set cursor after @
                            const range = document.createRange();
                            const sel = window.getSelection();
                            range.setStart(input.firstChild, 5);
                            range.setEnd(input.firstChild, 5);
                            sel.removeAllRanges();
                            sel.addRange(range);
                        } else if (selected.startsWith('/ephemeral')) {
                            input.innerHTML = '/ephemeral&nbsp;';
                            input.focus();
                            // Set cursor at end
                            const range = document.createRange();
                            const sel = window.getSelection();
                            range.selectNodeContents(input);
                            range.collapse(false);
                            sel.removeAllRanges();
                            sel.addRange(range);
                        }
                        input.focus();
                    });
                }
            }
        }
        if (event.key === 'Enter' && !event.shiftKey) {
            event.preventDefault();
            const text = input.textContent.trim();
            if (text !== '' && socket) {
                let msg;

                if (text.startsWith('/pm ')) {
                    // Format: /pm @username message
                    if (lastSelectedUsername) {
                        // Use the selected username
                        const parts = text.substring(4).match(/^@\S+\s+(.*)/);
                        if (parts) {
                            msg = {
                                type: "private",
                                metadata: { session_id: currentSessionToken, to_username: lastSelectedUsername },
                                content: parts[1]
                            };
                        }
                    } else {
                        const parts = text.substring(4).match(/^@(\S+)\s+(.*)/);
                        if (parts) {
                            msg = {
                                type: "private",
                                metadata: { session_id: currentSessionToken, to_username: parts[1] },
                                content: parts[2]
                            };
                        }
                    }
                    lastSelectedUsername = null; // Reset
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
                input.innerHTML = '';
            }
        }
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

document.addEventListener('click', (event) => {
    if (suggestionMenu && !suggestionMenu.contains(event.target) && event.target !== input) {
        hideSuggestions();
    }
});

checkSession();
