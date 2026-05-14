# dump_persons.ps1
$db = "c:\Users\carlj\projects\gallery-dl-api\gallery.db"
# Use a temporary file to dump
$sql = "SELECT p.name, pa.alias FROM persons p LEFT JOIN person_aliases pa ON p.id = pa.person_id WHERE p.name LIKE '%Fae%' OR pa.alias LIKE '%Fae%';"
# Actually I don't have sqlite3. I'll use the API but maybe I should check the code for list_persons.
