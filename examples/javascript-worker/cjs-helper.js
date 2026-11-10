"use strict";

function trim(value) {
    return value.trim();
}

function normalize(value) {
    return trim(value).toLowerCase();
}

module.exports = { normalize };
