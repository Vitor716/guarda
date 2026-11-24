package com.app;

import com.app.infra.HttpClient;

class NotaTest { void t() { new HttpClient().buscar().itens().get(0); } }
