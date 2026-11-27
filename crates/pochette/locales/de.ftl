# Die Texte von Pochette auf dem Desktop, auf Deutsch.
# Längen, Anzahlen und Daten stehen in den Texten des Kerns, in crates/pochette-core/locales.

## Das Hauptfenster

search-qobuz = Qobuz durchsuchen
main-menu = Hauptmenü
now-playing = Aktuelle Wiedergabe
tab-discover = Entdecken
tab-albums = Alben
menu-stats = Hörstatistik
menu-sign-out = Abmelden
previews-only = Mit diesem Konto kannst du nur 30-sekündige Hörproben abspielen.

## Start und Anmeldung

unreachable = Qobuz nicht erreichbar
try-again = Erneut versuchen
sign-in-tagline = Deine Qobuz-Bibliothek, Album für Album.
sign-in = Mit Qobuz anmelden
sign-in-hint =
    Zum Anmelden öffnet sich Qobuz in deinem Browser.
    Ein Qobuz-Abo ist erforderlich.
sign-in-continue = Im Browser fortfahren
sign-in-continue-hint =
    Melde dich im gerade geöffneten Browser-Tab bei Qobuz an.
    Pochette macht weiter, sobald du fertig bist.
sign-in-open-again = Browser erneut öffnen
sign-in-copy-link = Link kopieren
sign-in-cancel = Abbrechen
session-expired = Deine Sitzung ist abgelaufen. Melde dich erneut an.
sign-in-cannot-save = Die Sitzung kann nicht gespeichert werden: { $error }
sign-in-timed-out = Die Anmeldung hat zu lange gedauert. Versuch es erneut.

## Einstellungen

settings = Einstellungen
settings-language = Sprache
settings-language-system = Systemstandard
settings-language-restart = Pochette verwendet sie beim nächsten Start.
settings-devices = Geräte

## Geräte, in den Einstellungen

devices-about = Gekoppelte Geräte teilen miteinander, was du hörst, sobald sie im selben Netzwerk sind. So stimmen Statistik und Wiedergaben überall.
devices-paired = Gekoppelte Geräte
devices-in-background = Weiter synchronisieren, wenn Pochette geschlossen ist
devices-pair-a-device = Gerät koppeln
devices-pair-hint = Öffne auch auf dem anderen Gerät die Einstellungen von Pochette, dann erscheint es hier.
devices-looking = Suche nach Geräten in diesem Netzwerk…
devices-by-address = Über Adresse koppeln
devices-address = Seine Adresse, z. B. 192.168.1.20
devices-pair = Koppeln
devices-firewall =
    Wenn sich die Geräte nicht finden, steht vielleicht das Netzwerk oder eine Firewall im Weg. Pochette braucht TCP-Port { $port } zum Synchronisieren und UDP-Port 5353, um Geräte zu finden.
devices-pair-with = Mit { $name } koppeln
devices-cancel = Abbrechen
devices-paired-with = Mit { $name } gekoppelt.
devices-not-paired = Nicht mit { $name } gekoppelt.
devices-declined = { $name } hat das Koppeln abgelehnt.
devices-pairing-failed = Koppeln mit { $name } nicht möglich: { $reason }
devices-syncing = Wird synchronisiert…
# $when: wie lange her, etwa „vor 3 Minuten“.
devices-synced = Synchronisiert { $when }
devices-synced-error = Synchronisiert { $when }. { $error }
devices-not-synced = Noch nicht synchronisiert
devices-forget = Entkoppeln
devices-forget-tooltip = Nicht mehr mit diesem Gerät synchronisieren
devices-cannot-change = Änderung nicht möglich: { $error }
devices-connecting = Verbindung wird hergestellt…
devices-confirm = Prüfe, ob { $name } denselben Code anzeigt, und kopple dann auf beiden Geräten.
devices-waiting = Warte auf Bestätigung von { $name }…
sync-no-bus = Zum Synchronisieren wird ein D-Bus-Sitzungsbus benötigt: { $error }
sync-cannot-start = Die Synchronisierung kann nicht starten: { $error }
sync-no-answer = keine Antwort erhalten

## Überall

back = Zurück
play = Abspielen
pause = Pause
previous = Vorheriger Titel
next = Nächster Titel
see-all = Alle anzeigen
favorite-add = Zu deinen Alben hinzufügen
favorite-remove = Aus deinen Alben entfernen
genres = Genres
genres-all = Alle Genres
decades = Jahrzehnte
decades-all = Alle Jahrzehnte
# Ein Jahrzehnt, nach seinem ersten Jahr: „1970er“.
decade = { $decade }er
sort-by = Sortierung: { $sort }, { $order }
sort-picked = { $sort }, { $order }

## Der Tab „Alben“

albums-cannot-load = Deine Alben konnten nicht geladen werden
albums-none = Keine gespeicherten Alben
albums-none-hint = Alben, die du auf Qobuz zu deinen Favoriten hinzufügst, erscheinen hier.
albums-search = Alben, Künstler, Labels suchen
albums-random = Zufälliges Album abspielen
albums-random-filtered = Zufälliges Album aus der Auswahl abspielen
albums-no-match = Keine passenden Alben
albums-clear-filters = Suche und Filter zurücksetzen

## Der Tab „Entdecken“

discover-cannot-load = Entdecken konnte nicht geladen werden
discover-section-cannot-load = Diese Alben konnten nicht geladen werden
discover-section-empty = Keine Alben vorhanden
discover-new-releases = Neuerscheinungen
discover-press-awards = Presseauszeichnungen
discover-ideal-discography = Ideale Diskografie

## Suche auf Qobuz

search-query = „{ $query }“
search-failed = Qobuz-Suche fehlgeschlagen
search-nothing-found = Nichts gefunden
search-no-albums = Keine Alben gefunden
search-artists = Künstler

## Ein Album

album-cannot-load = Die Titel konnten nicht geladen werden: { $error }
album-about = Über dieses Album
album-copy-link = Link kopieren
album-link-copied = Link kopiert
# $plays: „12 Wiedergaben“, $last: wann, etwa „gestern“.
album-played = { $plays } · zuletzt { $last } gehört
hi-res = Hi-Res
album-disc = Disc { $number }
track-unavailable = Nicht zum Streamen verfügbar

## Ein Künstler

artist-kicker = Künstler
artist-more = Mehr
artist-less = Weniger
artist-cannot-load = Der Künstler konnte nicht geladen werden: { $error }
artist-no-albums = Von diesem Künstler gibt es keine Alben zum Streamen.
artist-similar = Ähnliche Künstler
releases-albums = Alben
releases-eps-singles = EPs & Singles
releases-live = Live
releases-compilations = Kompilationen
releases-other = Weitere Veröffentlichungen
releases-cannot-load = Diese Alben konnten nicht geladen werden
releases-empty = Keine Alben vorhanden

## Aktuelle Wiedergabe

now-playing-quality = Qualität
now-playing-show-cover = Cover anzeigen
now-playing-show-lyrics = Songtext anzeigen
now-playing-position = Position
now-playing-volume = Lautstärke
now-playing-tracks = Titel
output = Ausgabe
output-default = Systemstandard
quality-best = Beste Qualität
quality-hint = Jeder Titel wird in seiner besten Qualität gestreamt, höchstens in dieser und soweit dein Abo es zulässt.
quality-mp3 = MP3 · 320 kbit/s
quality-cd = CD · 16 Bit / 44,1 kHz
quality-hi-res-96 = Hi-Res · bis 24 Bit / 96 kHz
quality-hi-res-192 = Hi-Res · bis 24 Bit / 192 kHz
quality-mp3-playing = MP3 · 320 kbit/s
playback-cannot-start = Die Wiedergabe kann nicht starten: { $error }
playback-cannot-stream = „{ $title }“ kann nicht gestreamt werden: { $error }
lyrics-failed = Der Songtext konnte nicht geladen werden
lyrics-none = Kein Songtext für diesen Titel
lyrics-instrumental = Instrumental

## Hörstatistik

stats-year = Jahr
stats-month = Monat
stats-all-time = Gesamt
stats-whole-year = Ganzes Jahr
stats-top-albums = Top-Alben
stats-top-artists = Top-Künstler
stats-top-genres = Top-Genres
stats-nothing = Noch nichts gehört
stats-nothing-hint = Hör ein paar Alben: Hier erscheinen, wie viel du hörst, und deine Top-Alben, -Künstler und -Genres.
stats-listened = Gehört
stats-album-plays = Albumwiedergaben
stats-artists = Künstler
stats-by-year = Nach Jahr
stats-by-month = Nach Monat
stats-by-day = Nach Tag
stats-day = { $day }. { $month }
stats-bar = { $name }: { $time }
stats-share = { $percent } %

## Track

track-heading = Ein Titel ganz allein?
track-albums-only = Pochette findet, dass Songs zu ihren Alben gehören, in der Reihenfolge, die die Künstler gewählt haben. Einen herauszunehmen wäre, als würdest du nur ein einziges Kapitel eines Romans lesen.
track-on-album = „{ $track }“ wartet auf { $album } auf dich, genau dort, wo die Künstler ihn hingelegt haben.
track-open-album = Album öffnen
track-cannot-load = Das Album zu diesem Titel lässt sich nicht finden: { $error }
