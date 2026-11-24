@file:Suppress("UNUSED")
package com.app.domain

import com.app.infra.http.HttpClient
import com.app.util.*
import com.app.util.formatar as fmt

class Sonda {
    @Suppress("UNCHECKED_CAST")
    fun f(a: Int?, b: Boolean, c: Boolean): Int {
        externo@ for (i in 1..10) {
            if (a!! > i && b || c) {
                continue@externo
            } else if (b) {
                break@externo
            } else {
                return f(a, b, c)
            }
        }
        return 0
    }
}
