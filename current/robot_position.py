import csv
import polars as pl
import json
import os
import seaborn as sns
import matplotlib.pyplot as plt

from pathlib import Path


def robot_size_to_csv():
    directory = os.path.join(Path.home(), 'clear-chick')
    json_file_path = os.path.join(directory, 'images/1e77b3ee-5d7a-4088-9785-db78d14385f4.json')

    csv_path = os.path.join(directory, 'data_of_sizes.csv')
    with open(csv_path, 'w', newline='') as csv_file:
        csv_writer = csv.writer(csv_file, delimiter=',')
        csv_writer.writerow(["width", "height", "jersey", "jersey_middle_x", "jersey_middle_y"])
        # get_robot_size(json_file_path, csv_writer)

        images_dir = os.path.join(directory, 'images')
        for json_file in os.listdir(images_dir):
            file_path = os.path.join(images_dir, json_file)
            if os.path.isfile(file_path) and file_path.endswith('.json'):
                print(f"{file_path = }")
                get_robot_size(file_path, csv_writer)

    return csv_path




def get_robot_size(json_file_path, csv_writer):
    with open(json_file_path) as f:
        json_data = json.load(f)

        jersey_list = list(filter(lambda object: object['class'] == 'Jersey', json_data))

        for object in json_data:
            print(f"{object = }")
            if object['class'] == 'Robot':
                upper_left_x = object['points'][0][0]
                lower_right_x = object['points'][1][0]
                upper_left_y = object['points'][0][1]
                lower_right_y = object['points'][1][1]

                width = lower_right_x - upper_left_x
                height = lower_right_y - upper_left_y

                jersey = False
                for jersey_json in jersey_list:
                    jersey_positions = jersey_json['points']
                    if jersey_positions[0][0] > upper_left_x and jersey_positions[1][0] < lower_right_x and jersey_positions[1][1] < lower_right_y and jersey_positions[0][1] > upper_left_y:
                        jersey = True

                        ## jersey middle coordinate in roboter box normed
                        jersey_middle_x = (jersey_positions[0][0] - upper_left_x + (jersey_positions[1][0] - jersey_positions[0][0]) / 2) / width
                        jersey_middle_y = (jersey_positions[0][1] - upper_left_y + (jersey_positions[1][1] - jersey_positions[0][1]) / 2) / height
                        csv_writer.writerow([width, height, jersey, jersey_middle_x, jersey_middle_y])

                if not jersey:
                    csv_writer.writerow([width, height, jersey])
                    jersey_middle_x = 0
                    jersey_middle_y = 0

                print("robot: ", object['points'], f"{width = }, {height = }, {jersey = }, {jersey_middle_x = }, {jersey_middle_y = }")


def plot_robot_sizes(csv_file_path):

    robot_size_to_csv()
    sns.set_theme(style="ticks")
    df = pl.read_csv(csv_file_path)
    print(df)

    fig, axs = plt.subplots(ncols=2)
    sns.scatterplot(df.to_pandas(), x="width", y="height", hue="jersey", ax=axs[0])
    axs[1].invert_yaxis()
    
    sns.scatterplot(df.to_pandas(), x="jersey_middle_x", y="jersey_middle_y", ax=axs[1], color="cyan")
    plt.show()


def main():
    csv_file_path = robot_size_to_csv()
    plot_robot_sizes(csv_file_path)


if __name__ == "__main__":
    main()
