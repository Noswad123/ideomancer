"use strict";

const { normalize } = require('./cjs-helper');

function handle(req) {
    return normalize(req.body.value);
}

// An injected-router registration pattern, not a runnable HTTP server.
// Full mounting, framework activation, and request execution remain unknown.
module.exports = router => {
    router.post("/normalize", handle);
};
