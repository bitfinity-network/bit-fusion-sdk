import fs from 'fs';
import path from 'path';

function ensureDirectoryExistence(filePath: string) {
  var dirname = path.dirname(filePath);
  if (fs.existsSync(dirname)) {
    return true;
  }
  ensureDirectoryExistence(dirname);
  fs.mkdirSync(dirname);
}

// write values to json file logs
const writeLogFile = (filePath: string, input: Object): void => {
  const fullPath = "../../interface/src/contracts" + filePath;

  console.log('writing log file...');

  try {
    ensureDirectoryExistence(fullPath);
    fs.writeFileSync(fullPath, JSON.stringify(input));
    console.log(`updated - successfully written in ${fullPath}!`);
  } catch (err) {
    console.error(err);
  }

};

export default writeLogFile;
