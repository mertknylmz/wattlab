'use strict';
const params = new URLSearchParams(window.location.search);
const message = document.getElementById('message');
if (params.get('message')) message.textContent = params.get('message');
if (params.get('error') === '1') message.className = 'error';
