import sys

patch_path = sys.argv[1]

# Read in the file
with open(patch_path, "r") as file:
  filedata = file.read()

# Replace the target string
filedata = filedata.replace("export const BRC20_GENESIS_BLOCK = 779832;", "export const BRC20_GENESIS_BLOCK = 0;")

# Write the file out again
with open(patch_path, "w") as file:
  file.write(filedata)
