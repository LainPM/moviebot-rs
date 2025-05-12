async function fetchShahidSearch(name) {
    const searchUrl = formatShahidSearchUrl(name);
    
    try {
        const response = await fetch(searchUrl, getHeaders());
        const data = await response.json();

        if (!data.productList || !data.productList.products.length) {
            throw new Error("No products found");
        }

        let playlistId = data.productList.products[0]?.season?.playlists[1]?.id;
        if (!playlistId) {
            throw new Error("Playlist ID not found");
        }

        console.log(`Found Playlist ID: ${playlistId}`);
        await fetchShahidPlaylist("4992279710951949922826573713");
    } catch (error) {
        console.error("Error fetching search results:", error.message);
    }
}

async function fetchShahidPlaylist(playlistId) {
    let pageNumber = 0;
    let allTitles = [];

    while (true) {
        const playlistUrl = formatShahidPlaylistUrl(playlistId, pageNumber);

        try {
            const response = await fetch(playlistUrl, getHeaders());
            const data = await response.json();

            if (!data.productList || !data.productList.products.length) {
                console.log("No more results. Stopping pagination.");
                break;
            }

            // Extracting titles from all products in the playlist
            const titles = data.productList.products.map(product => product.title);
            allTitles.push(...titles); // Collect all titles across pages

            console.log(`Page ${pageNumber} Titles:`, titles);

            pageNumber++; // Move to the next page
        } catch (error) {
            console.error("Error fetching playlist data:", error.message);
            break;
        }
    }

    console.log("All Playlist Titles:", allTitles);
    return allTitles;
}

// Function to format the search URL
function formatShahidSearchUrl(name, pageNumber = 0, pageSize = 24, exactMatch = false, country = "EG") {
    const requestObject = { name, pageNumber, pageSize };
    const encodedRequest = encodeURIComponent(JSON.stringify(requestObject));
    return `https://api3.shahid.net/proxy/v2.1/t-search?request=${encodedRequest}&exactMatch=${exactMatch}&country=${country}`;
}

// Function to format the playlist URL
function formatShahidPlaylistUrl(playListId, pageNumber = 0, pageSize = 6, country = "EG") {
    const requestObject = {
        pageNumber,
        pageSize,
        playListId,
        sorts: [{ order: "DESC", type: "SORTDATE" }],
        isDynamicPlaylist: false
    };

    const encodedRequest = encodeURIComponent(JSON.stringify(requestObject));
    return `https://api3.shahid.net/proxy/v2.1/product/playlist?request=${encodedRequest}&country=${country}`;
}

// Function to return headers
function getHeaders() {
    return {
        headers: {
            "accept": "application/json, text/plain, */*",
            "accept-language": "en",
            "browser_name": "CHROME",
            "browser_version": "114.0.0.0",
            "cache-control": "no-cache",
            "language": "EN",
            "os_version": "NT 10.0",
            "pragma": "no-cache",
            "priority": "u=1, i",
            "sec-ch-ua": "\"Chromium\";v=\"128\", \"Not;A=Brand\";v=\"24\", \"Opera GX\";v=\"114\"",
            "sec-ch-ua-mobile": "?0",
            "sec-ch-ua-platform": "\"Windows\"",
            "sec-fetch-dest": "empty",
            "sec-fetch-mode": "cors",
            "sec-fetch-site": "cross-site",
            "shahid_os": "WEB",
        },
        referrer: "https://shahid.mbc.net/",
        referrerPolicy: "strict-origin-when-cross-origin",
        method: "GET",
        mode: "cors",
        credentials: "omit"
    };
}

// Start the process
fetchShahidSearch("baba geh");
