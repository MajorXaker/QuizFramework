// Quiz Helper frontend.
// Single-page app, served alongside the Rust backend.

(() => {
    "use strict";

    const state = {
        backendBase: "", // resolved from /api/config; empty = same origin
        sessionId: null,
        participantId: null,
        hostId: null, // only set if we are the host (== participantId for hosts)
        isHost: false,
        ws: null,
        wsClosing: false,
        currentRound: null,
        allowAnswerAt: null, // performance.now() at AllowAnswer receipt
        winner: null,
        joinsEnabled: true,
        answersEnabled: false,
        inviteCode: "",
        participants: [],
        myName: "",
    };

    // ---------- Element refs ----------
    const $ = (id) => document.getElementById(id);

    const views = {
        landing: $("view-landing"),
        player: $("view-player"),
        host: $("view-host"),
    };

    function showView(name) {
        for (const k of Object.keys(views)) {
            views[k].classList.toggle("active", k === name);
        }
    }

    // ---------- HTTP helpers ----------
    function apiUrl(path) {
        const base = state.backendBase || "";
        return `${base}${path}`;
    }
    function wsUrl(path) {
        const base = state.backendBase;
        if (base) {
            const u = new URL(path, base);
            u.protocol = u.protocol === "https:" ? "wss:" : "ws:";
            return u.toString();
        }
        const proto = location.protocol === "https:" ? "wss:" : "ws:";
        return `${proto}//${location.host}${path}`;
    }

    async function api(path, body) {
        const opts = { method: "POST", headers: { "content-type": "application/json" } };
        if (body !== undefined) opts.body = JSON.stringify(body);
        const r = await fetch(apiUrl(path), opts);
        const data = await r.json().catch(() => ({}));
        if (!r.ok) throw new Error(data.error || `HTTP ${r.status}`);
        return data;
    }

    async function loadConfig() {
        try {
            const r = await fetch(apiUrl("/api/config"));
            if (r.ok) {
                const cfg = await r.json();
                if (cfg.backend_url) state.backendBase = cfg.backend_url.replace(/\/$/, "");
            }
        } catch (e) { /* fall through to same-origin */ }
    }

    // ---------- Persistence ----------
    const SS_KEY = "quiz-helper-session";
    function saveSession() {
        sessionStorage.setItem(SS_KEY, JSON.stringify({
            sessionId: state.sessionId,
            participantId: state.participantId,
            hostId: state.hostId,
            isHost: state.isHost,
            myName: state.myName,
        }));
    }
    function clearSession() { sessionStorage.removeItem(SS_KEY); }
    function loadSession() {
        try {
            const raw = sessionStorage.getItem(SS_KEY);
            return raw ? JSON.parse(raw) : null;
        } catch { return null; }
    }

    // ---------- Landing handlers ----------
    $("form-host").addEventListener("submit", async (e) => {
        e.preventDefault();
        const name = $("host-name").value.trim();
        if (!name) return;
        try {
            const r = await api("/api/sessions", { host_name: name });
            state.sessionId = r.session_id;
            state.participantId = r.participant_id;
            state.hostId = r.participant_id;
            state.isHost = true;
            state.inviteCode = r.invite_code;
            state.myName = name;
            saveSession();
            connect();
        } catch (err) {
            $("landing-error").textContent = err.message;
        }
    });

    $("form-join").addEventListener("submit", async (e) => {
        e.preventDefault();
        const name = $("join-name").value.trim();
        const code = $("join-code").value.trim().toLowerCase();
        if (!name || !/^[a-z0-9]{8}$/.test(code)) {
            $("landing-error").textContent = t("landing.error.invalid_code");
            return;
        }
        try {
            const r = await api("/api/sessions/join", { invite_code: code, display_name: name });
            state.sessionId = r.session_id;
            state.participantId = r.participant_id;
            state.isHost = false;
            state.myName = name;
            saveSession();
            connect();
        } catch (err) {
            $("landing-error").textContent = err.message;
        }
    });

    // ---------- WebSocket ----------
    function connect() {
        const url = `${wsUrl("/ws")}?session_id=${encodeURIComponent(state.sessionId)}&participant_id=${encodeURIComponent(state.participantId)}`;
        state.wsClosing = false;
        const ws = new WebSocket(url);
        state.ws = ws;
        ws.addEventListener("open", () => {
            showView(state.isHost ? "host" : "player");
            const suffix = state.isHost ? t("host.name_suffix") : t("player.name_suffix");
            $(state.isHost ? "host-who" : "player-who").textContent = `${state.myName}${suffix}`;
        });
        ws.addEventListener("message", (e) => {
            let msg;
            try { msg = JSON.parse(e.data); } catch { return; }
            handleServerMsg(msg);
        });
        ws.addEventListener("close", () => {
            state.ws = null;
            if (!state.wsClosing) {
                // Unexpected disconnect — go back to landing.
                clearSession();
                showView("landing");
                $("landing-error").textContent = t("landing.error.disconnected");
            }
        });
    }

    function send(obj) {
        if (state.ws && state.ws.readyState === WebSocket.OPEN) {
            state.ws.send(JSON.stringify(obj));
        }
    }

    function leaveAndReset() {
        state.wsClosing = true;
        if (state.ws) state.ws.close();
        clearSession();
        Object.assign(state, {
            sessionId: null, participantId: null, hostId: null, isHost: false,
            ws: null, currentRound: null, allowAnswerAt: null, winner: null,
            inviteCode: "", participants: [], myName: "",
        });
        showView("landing");
        $("landing-error").textContent = "";
    }

    $("player-leave").addEventListener("click", leaveAndReset);

    $("host-close").addEventListener("click", async () => {
        if (!confirm(t("host.confirm_end_game"))) return;
        try { await api(`/api/sessions/${state.sessionId}/close`, { host_id: state.hostId }); } catch {}
        leaveAndReset();
    });

    // ---------- Server message router ----------
    function handleServerMsg(msg) {
        switch (msg.type) {
            case "welcome":
                state.inviteCode = msg.invite_code;
                state.joinsEnabled = msg.joins_enabled;
                state.answersEnabled = msg.answers_enabled;
                state.participants = msg.participants;
                renderAll();
                break;
            case "session_state":
                state.inviteCode = msg.invite_code;
                state.joinsEnabled = msg.joins_enabled;
                state.answersEnabled = msg.answers_enabled;
                renderAll();
                break;
            case "participants":
                state.participants = msg.participants;
                renderRosters();
                break;
            case "allow_answer":
                state.currentRound = msg.round_id;
                state.allowAnswerAt = performance.now();
                state.winner = null;
                renderRoundUi();
                break;
            case "stop_answer":
                state.currentRound = null;
                state.allowAnswerAt = null;
                if (msg.winner) state.winner = msg.winner;
                renderRoundUi();
                break;
            case "answer_winner":
                state.winner = msg.winner;
                renderRoundUi();
                break;
            case "kicked":
                alert(t("session.kicked"));
                leaveAndReset();
                break;
            case "session_closed":
                alert(t("session.closed", { reason: msg.reason }));
                leaveAndReset();
                break;
            case "error":
                console.warn("server error:", msg.message);
                break;
            case "pong":
                break;
        }
    }

    // ---------- Rendering ----------
    function renderAll() {
        renderInvite();
        renderToggles();
        renderRosters();
        renderRoundUi();
    }

    function renderInvite() {
        $("invite-code-display").textContent = state.inviteCode;
        $("config-code-display").textContent = state.inviteCode;
    }

    function renderToggles() {
        $("toggle-joins").checked = state.joinsEnabled;
        $("toggle-answers").checked = state.answersEnabled;
    }

    function renderRosters() {
        const players = state.participants.filter((p) => !p.is_host);

        // Host views (config + game)
        for (const id of ["host-roster", "config-roster"]) {
            const el = $(id);
            el.innerHTML = "";
            for (const p of players) {
                const li = document.createElement("li");
                li.className = "player";

                const left = document.createElement("div");
                const dot = document.createElement("span");
                dot.className = "dot" + (p.connected ? " online" : "");
                const name = document.createElement("span");
                name.className = "name";
                name.textContent = p.name;
                left.appendChild(dot);
                left.appendChild(name);

                const right = document.createElement("button");
                right.className = "kick";
                right.textContent = t("host.roster.kick");
                right.addEventListener("click", () => kickPlayer(p.id));
                li.appendChild(left);
                li.appendChild(right);
                el.appendChild(li);
            }
            if (players.length === 0) {
                const empty = document.createElement("li");
                empty.className = "muted";
                empty.textContent = t("host.roster.empty");
                el.appendChild(empty);
            }
        }

        // Player view roster (read-only)
        const pr = $("player-roster");
        pr.innerHTML = "";
        for (const p of state.participants) {
            const li = document.createElement("li");
            li.className = "player";
            const left = document.createElement("div");
            const dot = document.createElement("span");
            dot.className = "dot" + (p.connected ? " online" : "");
            const name = document.createElement("span");
            name.className = "name";
            // The "(host)" suffix is defined in the locale so it can be translated.
            name.textContent = p.name + (p.is_host ? t("host.name_suffix") : "");
            left.appendChild(dot);
            left.appendChild(name);
            li.appendChild(left);
            pr.appendChild(li);
        }
    }

    function renderRoundUi() {
        if (state.isHost) {
            $("btn-start-round").disabled = state.currentRound !== null;
            $("btn-stop-round").disabled = state.currentRound === null;
            const banner = $("host-winner");
            if (state.winner) {
                banner.classList.remove("hidden");
                banner.classList.add("win");
                banner.classList.remove("lose");
                banner.textContent = t("host.winner_banner.buzzed_first", {
                    name: state.winner.participant_name,
                    ms: state.winner.elapsed_ms,
                });
            } else if (state.currentRound) {
                banner.classList.remove("hidden");
                banner.classList.remove("win");
                banner.classList.add("lose");
                banner.textContent = t("host.winner_banner.waiting");
            } else {
                banner.classList.add("hidden");
            }
            $("host-status").textContent = state.currentRound
                ? t("host.status.live")
                : (state.answersEnabled
                    ? t("host.status.answer_mode_on")
                    : t("host.status.answer_mode_off"));
        } else {
            const btn = $("answer-btn");
            const banner = $("winner-banner");
            const status = $("player-status");

            if (state.winner) {
                btn.disabled = true;
                btn.classList.remove("armed");
                banner.classList.remove("hidden");
                if (state.winner.participant_id === state.participantId) {
                    banner.classList.add("win");
                    banner.classList.remove("lose");
                    banner.textContent = t("player.winner_banner.you_were_first", {
                        ms: state.winner.elapsed_ms,
                    });
                } else {
                    banner.classList.add("lose");
                    banner.classList.remove("win");
                    banner.textContent = t("player.winner_banner.other_was_first", {
                        name: state.winner.participant_name,
                        ms: state.winner.elapsed_ms,
                    });
                }
                status.textContent = "";
            } else if (state.currentRound) {
                btn.disabled = false;
                btn.classList.add("armed");
                banner.classList.add("hidden");
                status.textContent = t("player.status.answer_now");
            } else if (state.answersEnabled) {
                btn.disabled = true;
                btn.classList.remove("armed");
                btn.textContent = t("player.buzz_button.wait");
                banner.classList.add("hidden");
                status.textContent = t("player.status.waiting_for_host");
            } else {
                btn.disabled = true;
                btn.classList.remove("armed");
                btn.textContent = t("player.buzz_button.default");
                banner.classList.add("hidden");
                status.textContent = t("player.status.answer_mode_off");
            }
        }
    }

    // ---------- Host actions ----------
    async function kickPlayer(participantId) {
        try {
            await api(`/api/sessions/${state.sessionId}/participants/${participantId}/kick`,
                { host_id: state.hostId });
        } catch (e) { console.warn(e); }
    }
    $("btn-regen-code").addEventListener("click", async () => {
        try { await api(`/api/sessions/${state.sessionId}/regenerate_code`, { host_id: state.hostId }); }
        catch (e) { alert(e.message); }
    });
    $("toggle-joins").addEventListener("change", async (e) => {
        try { await api(`/api/sessions/${state.sessionId}/joins_enabled`, { host_id: state.hostId, enabled: e.target.checked }); }
        catch (err) { alert(err.message); e.target.checked = !e.target.checked; }
    });
    $("toggle-answers").addEventListener("change", async (e) => {
        try { await api(`/api/sessions/${state.sessionId}/answers_enabled`, { host_id: state.hostId, enabled: e.target.checked }); }
        catch (err) { alert(err.message); e.target.checked = !e.target.checked; }
    });
    $("btn-start-round").addEventListener("click", async () => {
        try { await api(`/api/sessions/${state.sessionId}/start_round`, { host_id: state.hostId }); }
        catch (e) { alert(e.message); }
    });
    $("btn-stop-round").addEventListener("click", async () => {
        try { await api(`/api/sessions/${state.sessionId}/stop_round`, { host_id: state.hostId }); }
        catch (e) { alert(e.message); }
    });

    // Tabs
    document.querySelectorAll(".tab").forEach((t) => {
        t.addEventListener("click", () => {
            document.querySelectorAll(".tab").forEach((x) => x.classList.remove("active"));
            t.classList.add("active");
            const name = t.dataset.tab;
            document.querySelectorAll(".tab-pane").forEach((p) => p.classList.remove("active"));
            $(`tab-${name}`).classList.add("active");
        });
    });

    // ---------- Player buzzer ----------
    const buzz = (e) => {
        if (e) e.preventDefault();
        if (!state.currentRound || !state.allowAnswerAt || state.winner) return;
        const elapsed = Math.max(0, Math.round(performance.now() - state.allowAnswerAt));
        send({ type: "answer", elapsed_ms: elapsed });
        // Optimistic local lockout — server is authoritative.
        const btn = $("answer-btn");
        btn.disabled = true;
        btn.classList.remove("armed");
        btn.textContent = t("player.buzz_button.sent", { ms: elapsed });
    };
    // Use pointerdown for lowest-latency input.
    $("answer-btn").addEventListener("pointerdown", buzz);

    // ---------- Boot ----------
    async function boot() {
        await loadConfig();
        const saved = loadSession();
        if (saved && saved.sessionId && saved.participantId) {
            Object.assign(state, saved);
            connect();
        } else {
            showView("landing");
        }
    }
    boot();
})();
