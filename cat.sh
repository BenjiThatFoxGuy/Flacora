#!/bin/sh
case "$1" in
    --search) title="Search: $2" ;;
    --browse) 
        # Determine what kind of page based on URL
        if echo "$2" | grep -q 'artist/'; then
            # Artist page: return albums
            echo "@flacora {\"event\":\"catalog\",\"page\":{\"title\":\"Artist: $2\",\"note\":\"synthetic\",\"items\":[{\"kind\":\"album\",\"title\":\"Album 1\",\"detail\":\"Synthetic\",\"url\":\"https://example.test/album/1\"},{\"kind\":\"album\",\"title\":\"Album 2\",\"detail\":\"Synthetic\",\"url\":\"https://example.test/album/2\"}],\"next\":\"cursor:album-page-1\",\"artwork\":null}}"
        elif echo "$2" | grep -q 'album/'; then
            # Album page: return tracks + next cursor to artist
            echo "@flacora {\"event\":\"catalog\",\"page\":{\"title\":\"Album: $2\",\"note\":\"synthetic\",\"items\":[{\"kind\":\"track\",\"title\":\"Track A\",\"detail\":\"Synthetic\",\"url\":\"https://example.test/track/1\"},{\"kind\":\"track\",\"title\":\"Track B\",\"detail\":\"Synthetic\",\"url\":\"https://example.test/track/2\"},{\"kind\":\"track\",\"title\":\"Track C\",\"detail\":\"Synthetic\",\"url\":\"https://example.test/track/3\"}],\"next\":\"cursor:artist-page\",\"artwork\":null}}"
        else
            title="Home"
        fi
        ;;
    *) title="Home" ;;
esac
