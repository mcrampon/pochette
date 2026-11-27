# Os textos do Pochette no desktop, em português do Brasil.
# Durações, contagens e datas ficam nos textos do núcleo, em crates/pochette-core/locales.

## A janela principal

search-qobuz = Buscar no Qobuz
main-menu = Menu principal
now-playing = Tocando agora
tab-discover = Descobrir
tab-albums = Álbuns
menu-stats = Estatísticas de audição
menu-sign-out = Sair
previews-only = Esta conta só pode tocar prévias de 30 segundos.

## Início e login

unreachable = Não foi possível conectar ao Qobuz
try-again = Tentar novamente
sign-in-tagline = Sua biblioteca do Qobuz, álbum por álbum.
sign-in = Entrar com o Qobuz
sign-in-hint =
    Ao entrar, o Qobuz abre no seu navegador.
    É preciso ter uma assinatura do Qobuz.
sign-in-continue = Continue no seu navegador
sign-in-continue-hint =
    Entre no Qobuz na aba do navegador que acabou de abrir.
    O Pochette continua assim que você terminar.
sign-in-open-again = Abrir o navegador novamente
sign-in-copy-link = Copiar link
sign-in-cancel = Cancelar
session-expired = Sua sessão expirou. Entre novamente.
sign-in-cannot-save = Não foi possível salvar a sessão: { $error }
sign-in-timed-out = O login demorou demais. Tente novamente.

## Configurações

settings = Configurações
settings-language = Idioma
settings-language-system = Padrão do sistema
settings-language-restart = O Pochette usará este idioma na próxima vez que for aberto.
settings-devices = Dispositivos

## Dispositivos, nas Configurações

devices-about = Os dispositivos pareados compartilham entre si o que você ouve, para as estatísticas e as contagens de reprodução, sempre que estão na mesma rede.
devices-paired = Dispositivos pareados
devices-in-background = Continuar sincronizando com o Pochette fechado
devices-pair-a-device = Parear um dispositivo
devices-pair-hint = Abra também as Configurações do Pochette no outro dispositivo, e ele aparecerá aqui.
devices-looking = Procurando dispositivos nesta rede…
devices-by-address = Parear pelo endereço
devices-address = O endereço dele, por exemplo 192.168.1.20
devices-pair = Parear
devices-firewall =
    Se os dispositivos não se encontrarem, a rede ou um firewall pode estar bloqueando. O Pochette precisa da porta TCP { $port } para sincronizar e da porta UDP 5353 para encontrar dispositivos.
devices-pair-with = Parear com { $name }
devices-cancel = Cancelar
devices-paired-with = Pareado com { $name }.
devices-not-paired = Não pareado com { $name }.
devices-declined = { $name } recusou o pareamento.
devices-pairing-failed = Não foi possível parear com { $name }: { $reason }
devices-syncing = Sincronizando…
devices-synced = Sincronizado { $when }
devices-synced-error = Sincronizado { $when }. { $error }
devices-not-synced = Ainda não sincronizado
devices-forget = Esquecer
devices-forget-tooltip = Parar de sincronizar com este dispositivo
devices-cannot-change = Não foi possível alterar: { $error }
devices-connecting = Conectando…
devices-confirm = Confira se { $name } mostra o mesmo código e confirme o pareamento nos dois.
devices-waiting = Aguardando a confirmação de { $name }…
sync-no-bus = A sincronização precisa de um barramento de sessão D-Bus: { $error }
sync-cannot-start = Não foi possível iniciar a sincronização: { $error }
sync-no-answer = não houve resposta

## Em todo lugar

back = Voltar
play = Tocar
pause = Pausar
previous = Anterior
next = Próxima
see-all = Ver tudo
favorite-add = Adicionar aos seus álbuns
favorite-remove = Remover dos seus álbuns
genres = Gêneros
genres-all = Todos os gêneros
decades = Décadas
decades-all = Todas as décadas
decade = Anos { $decade }
sort-by = Ordenação: { $sort } · { $order }
sort-picked = { $sort } · { $order }

## A aba Álbuns

albums-cannot-load = Não foi possível carregar seus álbuns
albums-none = Nenhum álbum salvo
albums-none-hint = Os álbuns que você adicionar aos favoritos no Qobuz aparecem aqui.
albums-search = Buscar álbuns, artistas, gravadoras
albums-random = Tocar um álbum aleatório
albums-random-filtered = Tocar um álbum aleatório destes
albums-no-match = Nenhum álbum encontrado
albums-clear-filters = Limpar busca e filtros

## A aba Descobrir

discover-cannot-load = Não foi possível carregar Descobrir
discover-section-cannot-load = Não foi possível carregar estes álbuns
discover-section-empty = Nenhum álbum aqui
discover-new-releases = Lançamentos
discover-press-awards = Prêmios da imprensa
discover-ideal-discography = Discoteca ideal

## Busca no Qobuz

search-query = “{ $query }”
search-failed = Não foi possível buscar no Qobuz
search-nothing-found = Nada encontrado
search-no-albums = Nenhum álbum encontrado
search-artists = Artistas

## Um álbum

album-cannot-load = Não foi possível carregar as faixas: { $error }
album-about = Sobre este álbum
album-played = { $plays } · ouvido pela última vez { $last }
hi-res = Hi-Res
album-disc = Disco { $number }
track-unavailable = Indisponível para streaming

## Um artista

artist-kicker = Artista
artist-more = Mais
artist-less = Menos
artist-cannot-load = Não foi possível carregar este artista: { $error }
artist-no-albums = Nenhum álbum deste artista disponível para streaming.
artist-similar = Artistas semelhantes
releases-albums = Álbuns
releases-eps-singles = EPs e singles
releases-live = Ao vivo
releases-compilations = Coletâneas
releases-other = Outros lançamentos
releases-cannot-load = Não foi possível carregar estes álbuns
releases-empty = Nenhum álbum aqui

## Tocando agora

now-playing-quality = Qualidade
now-playing-show-cover = Mostrar a capa
now-playing-show-lyrics = Mostrar a letra
now-playing-position = Posição
now-playing-volume = Volume
now-playing-tracks = Faixas
output = Saída
output-default = Padrão do sistema
quality-best = Melhor qualidade
quality-hint = Cada faixa toca na melhor qualidade disponível, até este limite, conforme sua assinatura permitir.
quality-mp3 = MP3 · 320 kbps
quality-cd = CD · 16 bits / 44,1 kHz
quality-hi-res-96 = Hi-Res · até 24 bits / 96 kHz
quality-hi-res-192 = Hi-Res · até 24 bits / 192 kHz
quality-mp3-playing = MP3 · 320 kbps
playback-cannot-start = Não foi possível iniciar a reprodução: { $error }
playback-cannot-stream = Não foi possível tocar “{ $title }”: { $error }
lyrics-failed = Não foi possível carregar a letra
lyrics-none = Esta faixa não tem letra
lyrics-instrumental = Instrumental

## Estatísticas de audição

stats-year = Ano
stats-month = Mês
stats-all-time = Todo o período
stats-whole-year = Ano inteiro
stats-top-albums = Álbuns mais ouvidos
stats-top-artists = Artistas mais ouvidos
stats-top-genres = Gêneros mais ouvidos
stats-nothing = Nada ouvido ainda
stats-nothing-hint = Ouça alguns álbuns: quanto você ouve e seus álbuns, artistas e gêneros mais ouvidos aparecem aqui.
stats-listened = Tempo ouvido
stats-album-plays = Reproduções de álbuns
stats-artists = Artistas
stats-by-year = Por ano
stats-by-month = Por mês
stats-by-day = Por dia
stats-day = { $day } de { $month }
stats-bar = { $name }: { $time }
stats-share = { $percent }%
