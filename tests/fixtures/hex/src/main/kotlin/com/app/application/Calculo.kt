package com.app.application

import com.app.domain.Nota

class Calculo {
    fun total(n: Nota?): Int {
        val a = n!!.itens().size
        // guarda:ignorar KT001 motivo: teste de supressão com motivo
        val b = n!!.cru().size
        // guarda:ignorar KT001
        return a + b + n!!.itens().size
    }
}
