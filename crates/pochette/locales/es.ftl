# Los textos de Pochette en el escritorio, en español.

## La ventana principal

search-qobuz = Buscar en Qobuz
main-menu = Menú principal
now-playing = En reproducción
tab-discover = Descubrir
tab-albums = Álbumes
menu-stats = Estadísticas de escucha
menu-sign-out = Cerrar sesión
previews-only = Esta cuenta solo puede reproducir fragmentos de 30 segundos.

## Inicio e inicio de sesión

unreachable = No se puede conectar con Qobuz
try-again = Reintentar
sign-in-tagline = Tu biblioteca de Qobuz, álbum a álbum.
sign-in = Iniciar sesión con Qobuz
sign-in-hint =
    Al iniciar sesión, Qobuz se abre en tu navegador.
    Requiere una suscripción a Qobuz.
sign-in-continue = Continúa en tu navegador
sign-in-continue-hint =
    Inicia sesión en Qobuz en la pestaña del navegador que se acaba de abrir.
    Pochette continuará en cuanto termines.
sign-in-open-again = Volver a abrir el navegador
sign-in-copy-link = Copiar enlace
sign-in-cancel = Cancelar
session-expired = Tu sesión ha caducado. Vuelve a iniciar sesión.
sign-in-cannot-save = No se puede guardar la sesión: { $error }
sign-in-timed-out = El inicio de sesión ha tardado demasiado. Inténtalo de nuevo.

## Ajustes

settings = Ajustes
settings-language = Idioma
settings-language-system = Idioma del sistema
settings-language-restart = Pochette usará este idioma la próxima vez que se abra.
settings-devices = Dispositivos

## Dispositivos, en Ajustes

devices-about = Los dispositivos vinculados comparten entre sí lo que escuchas, para las estadísticas y los recuentos de reproducciones, siempre que estén en la misma red.
devices-paired = Dispositivos vinculados
devices-in-background = Seguir sincronizando con Pochette cerrado
devices-pair-a-device = Vincular un dispositivo
devices-pair-hint = Abre también los ajustes de Pochette en el otro dispositivo y aparecerá aquí.
devices-looking = Buscando dispositivos en esta red…
devices-by-address = Vincular por dirección
devices-address = Su dirección, por ejemplo 192.168.1.20
devices-pair = Vincular
devices-firewall =
    Si los dispositivos no se encuentran, puede que la red o un cortafuegos lo impidan. Pochette necesita el puerto TCP { $port } para sincronizar y el puerto UDP 5353 para encontrar dispositivos.
devices-pair-with = Vincular con { $name }
devices-cancel = Cancelar
devices-paired-with = Vinculado con { $name }.
devices-not-paired = No vinculado con { $name }.
devices-declined = { $name } ha rechazado la vinculación.
devices-pairing-failed = No se puede vincular con { $name }: { $reason }
devices-syncing = Sincronizando…
devices-synced = Sincronizado { $when }
devices-synced-error = Sincronizado { $when }. { $error }
devices-not-synced = Aún no sincronizado
devices-forget = Olvidar
devices-forget-tooltip = Dejar de sincronizar con este dispositivo
devices-cannot-change = No se puede cambiar: { $error }
devices-connecting = Conectando…
devices-confirm = Comprueba que { $name } muestra el mismo código y vincula en los dos.
devices-waiting = Esperando a que { $name } confirme…
sync-no-bus = Para sincronizar hace falta un bus de sesión D-Bus: { $error }
sync-cannot-start = No se puede iniciar la sincronización: { $error }
sync-no-answer = no ha respondido

## En todas partes

back = Atrás
play = Reproducir
pause = Pausar
previous = Anterior
next = Siguiente
see-all = Ver todo
favorite-add = Añadir a tus álbumes
favorite-remove = Quitar de tus álbumes
genres = Géneros
genres-all = Todos los géneros
decades = Décadas
decades-all = Todas las décadas
# Una década, por su primer año: «Años 1970».
decade = Años { $decade }
sort-by = Orden: { $sort } ({ $order })
sort-picked = { $sort } ({ $order })

## La pestaña Álbumes

albums-cannot-load = No se pueden cargar tus álbumes
albums-none = No hay álbumes guardados
albums-none-hint = Aquí aparecen los álbumes que añadas a tus favoritos en Qobuz.
albums-search = Buscar álbumes, artistas, sellos
albums-random = Reproducir un álbum al azar
albums-random-filtered = Reproducir uno de estos álbumes al azar
albums-no-match = Ningún álbum coincide
albums-clear-filters = Borrar búsqueda y filtros

## La pestaña Descubrir

discover-cannot-load = No se puede cargar Descubrir
discover-section-cannot-load = No se pueden cargar estos álbumes
discover-section-empty = No hay álbumes aquí
discover-new-releases = Novedades
discover-press-awards = Premios de la prensa
discover-ideal-discography = Discografía ideal

## Buscar en Qobuz

search-query = «{ $query }»
search-failed = No se puede buscar en Qobuz
search-nothing-found = No se ha encontrado nada
search-no-albums = No se han encontrado álbumes
search-artists = Artistas

## Un álbum

album-cannot-load = No se pueden cargar las pistas: { $error }
album-about = Acerca de este álbum
# $plays es «12 reproducciones», $last cuándo, como «ayer».
album-played = { $plays } · última reproducción { $last }
hi-res = Hi-Res
album-disc = Disco { $number }
track-unavailable = No disponible en streaming

## Un artista

artist-kicker = Artista
artist-more = Más
artist-less = Menos
artist-cannot-load = No se puede cargar este artista: { $error }
artist-no-albums = Este artista no tiene álbumes disponibles en streaming.
artist-similar = Artistas similares
releases-albums = Álbumes
releases-eps-singles = EP y singles
releases-live = En directo
releases-compilations = Recopilatorios
releases-other = Otros lanzamientos
releases-cannot-load = No se pueden cargar estos álbumes
releases-empty = No hay álbumes aquí

## En reproducción

now-playing-quality = Calidad
now-playing-show-cover = Mostrar la portada
now-playing-show-lyrics = Mostrar la letra
now-playing-position = Posición
now-playing-volume = Volumen
now-playing-tracks = Pistas
output = Salida
output-default = Predeterminada del sistema
quality-best = Máxima calidad
quality-hint = Cada pista se reproduce en la mejor calidad que tenga, hasta esta, según lo permita tu suscripción.
quality-mp3 = MP3 · 320 kbps
quality-cd = CD · 16 bits / 44,1 kHz
quality-hi-res-96 = Hi-Res · hasta 24 bits / 96 kHz
quality-hi-res-192 = Hi-Res · hasta 24 bits / 192 kHz
quality-mp3-playing = MP3 · 320 kbps
playback-cannot-start = No se puede iniciar la reproducción: { $error }
playback-cannot-stream = No se puede reproducir «{ $title }»: { $error }
lyrics-failed = No se puede cargar la letra
lyrics-none = No hay letra para esta pista
lyrics-instrumental = Instrumental

## Estadísticas de escucha

stats-year = Año
stats-month = Mes
stats-all-time = Desde siempre
stats-whole-year = Todo el año
stats-top-albums = Álbumes más escuchados
stats-top-artists = Artistas más escuchados
stats-top-genres = Géneros más escuchados
stats-nothing = Aún no has escuchado nada
stats-nothing-hint = Escucha algunos álbumes: aquí aparecerán cuánto escuchas y tus álbumes, artistas y géneros más escuchados.
stats-listened = Tiempo de escucha
stats-album-plays = Reproducciones de álbumes
stats-artists = Artistas
stats-by-year = Por año
stats-by-month = Por mes
stats-by-day = Por día
stats-day = { $day } de { $month }
stats-bar = { $name }: { $time }
stats-share = { $percent } %
