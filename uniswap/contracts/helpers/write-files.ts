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
  const fullPath = './logs/' + filePath;

  console.log('writing log file...');
  if (fs.existsSync(fullPath)) {
    try {
      ensureDirectoryExistence(fullPath);
      fs.appendFileSync(fullPath, JSON.stringify(input));
      console.log(`appended - successfully written in ${fullPath}!`);
    } catch (err) {
      console.error(err);
    }
  } else {
    try {
      ensureDirectoryExistence(fullPath);
      fs.writeFileSync(fullPath, JSON.stringify(input));
      console.log(`created - successfully written in ${fullPath}!`);
    } catch (err) {
      console.error(err);
    }
  }
};

export default writeLogFile;
