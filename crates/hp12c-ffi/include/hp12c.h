/* API C do núcleo da HP-12C (hp12c_ffi.dll). Versão da API: 1.
 *
 * - A calculadora é um ponteiro opaco: crie com hp12c_new/hp12c_load_state e libere com hp12c_free.
 * - Teclas: índices 0–38 na ordem física, linha a linha (0 = n, 9 = ÷, 25 = ENTER, 30 = ON …).
 * - Textos são UTF-8. Funções que devolvem texto escrevem em `buffer` (até `capacity` bytes,
 *   sempre com '\0') e retornam o tamanho completo; passe buffer NULL para saber o tamanho.
 */
#ifndef HP12C_H
#define HP12C_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct Hp12c Hp12c;

/* Bits de Hp12cDisplay.annunciators */
#define HP12C_ANN_LOW_BATTERY (1u << 0)
#define HP12C_ANN_F           (1u << 1)
#define HP12C_ANN_G           (1u << 2)
#define HP12C_ANN_BEGIN       (1u << 3)
#define HP12C_ANN_DMY         (1u << 4)
#define HP12C_ANN_COMPOUND    (1u << 5)
#define HP12C_ANN_PRGM        (1u << 6)

typedef struct Hp12cDisplay {
    uint8_t  minus;        /* 1 = sinal de menos aceso */
    uint8_t  blinking;     /* 1 = piscando ("running") */
    uint8_t  annunciators; /* bits HP12C_ANN_* */
    uint8_t  reserved;
    uint32_t glyphs[10];   /* caractere de cada posição (Unicode; ' ' = apagado) */
    uint8_t  punct[10];    /* 0 nenhuma, 1 ponto, 2 vírgula */
} Hp12cDisplay;

uint32_t hp12c_api_version(void);

Hp12c*  hp12c_new(void);
void    hp12c_free(Hp12c* calc);
Hp12c*  hp12c_load_state(const char* json);   /* NULL se inválido */

void    hp12c_press(Hp12c* calc, uint8_t key_index);
void    hp12c_release(Hp12c* calc, uint8_t key_index);
void    hp12c_tick(Hp12c* calc, uint32_t elapsed_ms);
uint8_t hp12c_is_running(const Hp12c* calc);
int32_t hp12c_tap_all(Hp12c* calc, const char* sequence);   /* 0 ok, -1 tecla desconhecida */
int32_t hp12c_key_from_token(const char* token);            /* índice ou -1 */

void    hp12c_display(const Hp12c* calc, Hp12cDisplay* out);
size_t  hp12c_display_text(const Hp12c* calc, char* buffer, size_t capacity);
size_t  hp12c_x_text(const Hp12c* calc, char* buffer, size_t capacity);
size_t  hp12c_save_state(const Hp12c* calc, char* buffer, size_t capacity);
size_t  hp12c_program_listing(const Hp12c* calc, char* buffer, size_t capacity);
int32_t hp12c_load_program(Hp12c* calc, const char* listing);   /* 0 ok, -1 inválido */

#ifdef __cplusplus
}
#endif

#endif /* HP12C_H */
