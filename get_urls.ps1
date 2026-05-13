# Define the path to your CSV file
$csvPath = "c:\Users\carlj\AppData\Local\Temp\7cc3933f-75c0-468b-aa03-25b76d9eb7d0_godownload.db_export.zip.7d0\sources.csv"

# Import the CSV and extract the second column (url)
$urls = Import-Csv -Path $csvPath | Select-Object -ExpandProperty url

# Display the URLs
$urls
