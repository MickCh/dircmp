1.
Chciałbym napisać aplikacje w Rust, która będzie korzystała z terminala (TUI). Czy jesteś mi w stanie z tym pomóc?

2.
Być może widziałaś w programie total commander narzędzie do porównywania folderów. Chcę zrobić coś podobnego, ale w terminalu. Ten program ma działać zarówno pod linuksem jak i windowsem.

3.
Teraz kwestia programu. Rust, najnowsza wersja, może być Ratatui.
Oczywiście SOLID i proszę Cię rozbij ten projekt na moduły logiczne. Typu warstwa prezentacji oddzielnie, silnik oddzielnie.
Kwestia porównywania plików powinna być zaimplementowana w taki sposób by można było ją elastycznie rozszerzyć - gdy np. zdecyduję się, że porównywanie plików tekstowych ma działać trochę inaczej (np. bez porównywania spacji).

Jeśli czegokolwiek Ci brakuje w moim opisie - pytaj. Zależy mi na tym, by ta aplikacja była napisana sensownie, żebym rzeczywiście mógł z niej wygodnie korzystać.

4.
Q: Co ma być podstawą porównywania plików? (Select all that apply)
A: Wszystkie naraz (konfigurowalne)

Q: Jakie typy plików chcesz obsługiwać ze specjalną logiką? (Select all that apply)
A: Pliki tekstowe (.txt, .md, .csv...)

Q: Jak głęboko ma się odbywać porównywanie folderów?
A: Rekurencyjnie (całe poddrzewo)

5.
Q: Jak ma wyglądać główny widok?
A: Dwa panele obok siebie (jak TC)

Q: Jakie akcje ma obsługiwać aplikacja? (Select all that apply)
A: Ten program będę rozwijał iteracyjnie. Najpierw skup się na porównaniu plików, o dodatkowej funkcjonalności pomyślimy później.

Q: Jak użytkownik ma konfigurować aplikację (np. metodę porównywania)?
A: Plik konfiguracyjny (TOML/JSON)

6.
Jest nieźle, co bym na pewno zmienił w interfejsie. Nie ma potrzeby pokazywania rozmiaru plików. Ramka dookoła może być, ale na środku chciałbym wyraźnie widzieć, jaki jest stan - tam ten środkowy fragment ramki tylko przeszkadza.
Gdy program startuje powinien od razu rozpocząc porównywanie. Po co to pytanie naciśnij F5, skoro wiadomo jaka akcja będzie wykonywana - to niepotrzebny krok dla uzytkownika.
